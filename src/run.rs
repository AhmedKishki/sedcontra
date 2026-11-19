//! `cross_examine`, one run from proposal to report, which every front end and the evaluation
//! share, and the replay of a recorded run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, anyhow, bail};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::actor::spawn_agent;
use crate::agents::{AgentDef, load_agent};
use crate::config::Config;
use crate::debate::{ADVERSARY_SHAPE, ADVOCATE_SHAPE, DebateInput, run_debate};
use crate::event::RunEvent;
use crate::provider::{ModelProvider, OpenAiCompatible, ReplayProvider};
use crate::record::RunRecord;
use crate::report::{Report, build_report, render_markdown, render_transcript};

pub struct Environment {
    pub config: Config,
    pub advocate: AgentDef,
    pub adversary: AgentDef,
    pub providers: BTreeMap<String, Arc<dyn ModelProvider>>,
}

impl Environment {
    pub fn load(config_path: Option<&Path>, adversary_file: Option<&Path>, advocate_file: Option<&Path>) -> anyhow::Result<Self> {
        let config = Config::load(config_path)?;
        let advocate = load_agent("advocate", &config, advocate_file, ADVOCATE_SHAPE)?;
        let adversary = load_agent("adversary", &config, adversary_file, ADVERSARY_SHAPE)?;
        let mut env = Environment { config, advocate, adversary, providers: BTreeMap::new() };
        for def in [env.advocate.clone(), env.adversary.clone()] {
            env.provider_for(&def)?;
        }
        Ok(env)
    }

    /// The provider the configuration assigns to an agent, built on first use.
    pub fn provider_for(&mut self, def: &AgentDef) -> anyhow::Result<Arc<dyn ModelProvider>> {
        let name = &def.model.provider;
        if let Some(provider) = self.providers.get(name) {
            return Ok(provider.clone());
        }
        let cfg = self.config.providers.get(name).ok_or_else(|| {
            anyhow!("the model of agent {} uses provider {name}, which [providers] does not define", def.name)
        })?;
        let provider: Arc<dyn ModelProvider> = Arc::new(OpenAiCompatible::from_config(name, cfg)?);
        self.providers.insert(name.clone(), provider.clone());
        Ok(provider)
    }
}

pub struct Progress {
    pub done: u32,
    pub max: u32,
    pub message: String,
}

pub struct RunOutput {
    pub report: Report,
    pub markdown: String,
    pub json: serde_json::Value,
}

pub enum RunFailure {
    Failed { reason: String, record_dir: Option<PathBuf>, transcript: String },
    Cancelled { record_dir: PathBuf },
}

pub async fn cross_examine(
    env: &Environment,
    proposal: String,
    context: Option<String>,
    progress: Option<mpsc::UnboundedSender<Progress>>,
    cancel: CancellationToken,
) -> Result<RunOutput, RunFailure> {
    let failed = |reason: String, record_dir: Option<PathBuf>| RunFailure::Failed {
        reason,
        record_dir,
        transcript: String::new(),
    };
    if proposal.trim().is_empty() {
        return Err(failed("the proposal is empty".into(), None));
    }
    let context = context.filter(|c| !c.trim().is_empty());
    let record = RunRecord::create(
        &env.config.runs_dir(),
        &proposal,
        context.as_deref(),
        &env.config,
        &[&env.advocate, &env.adversary],
    )
    .map_err(|e| failed(format!("cannot create the run record: {e:#}"), None))?;

    let provider = |def: &AgentDef| env.providers.get(&def.model.provider).cloned();
    let (Some(advocate_provider), Some(adversary_provider)) = (provider(&env.advocate), provider(&env.adversary))
    else {
        return Err(failed("no provider was built for the agents".into(), Some(record.dir.clone())));
    };
    let advocate = spawn_agent(env.advocate.clone(), advocate_provider);
    let adversary = spawn_agent(env.adversary.clone(), adversary_provider);

    let (tx, mut rx) = mpsc::unbounded_channel::<RunEvent>();
    let forward_record = record.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                RunEvent::Log(line) => {
                    if let Err(e) = forward_record.append_log(&line) {
                        eprintln!("warning: could not record the event log: {e:#}");
                    }
                }
                RunEvent::Turn(ev) => {
                    if let Err(e) = forward_record.write_turn(&ev) {
                        eprintln!("warning: could not record turn {}: {e:#}", ev.index);
                    }
                    if let Some(progress) = &progress {
                        let message = format!("round {}: {} done", ev.turn.round(), ev.turn.role());
                        let _ = progress.send(Progress { done: ev.index, max: ev.max, message });
                    }
                }
            }
        }
    });

    let input = DebateInput { proposal: proposal.clone(), context, round_limit: env.config.round_limit() };
    let result = run_debate(&input, &advocate, &adversary, Some(&tx), &cancel).await;
    drop(tx);
    let _ = forwarder.await;

    match result {
        Ok(outcome) => {
            let report = build_report(&record, outcome);
            let markdown = render_markdown(&report);
            let json = serde_json::to_value(&report).unwrap_or_default();
            if let Err(e) = record.write_report(&markdown, &json).and_then(|_| record.set_status("done")) {
                eprintln!("warning: could not write the report into {}: {e:#}", record.dir.display());
            }
            Ok(RunOutput { report, markdown, json })
        }
        Err(e) if e.cancelled => {
            let _ = record.set_status("cancelled");
            Err(RunFailure::Cancelled { record_dir: record.dir })
        }
        Err(e) => {
            let _ = record.set_status(&format!("failed: {}", e.reason));
            Err(RunFailure::Failed {
                reason: e.reason,
                record_dir: Some(record.dir),
                transcript: render_transcript(&e.turns),
            })
        }
    }
}

/// The parts of two run records that differ, empty when the runs ended alike.
pub fn compare_records(recorded: &Path, replayed: &Path) -> anyhow::Result<Vec<String>> {
    let mut differing = Vec::new();
    if read_status(recorded)? != read_status(replayed)? {
        differing.push("status".to_string());
    }
    match (read_report(recorded)?, read_report(replayed)?) {
        (None, None) => {}
        (Some(a), Some(b)) => {
            // `run_id` and `record_dir` are left out because they differ between any two runs.
            for part in ["ending", "rounds_run", "round_limit", "threads", "transcript"] {
                if a.get(part) != b.get(part) {
                    differing.push(part.to_string());
                }
            }
        }
        _ => differing.push("report".to_string()),
    }
    Ok(differing)
}

fn read_status(dir: &Path) -> anyhow::Result<String> {
    let path = dir.join("status");
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(text.trim().to_string())
}

fn read_report(dir: &Path) -> anyhow::Result<Option<serde_json::Value>> {
    let path = dir.join("report.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let report = serde_json::from_str(&text).with_context(|| format!("{} does not parse", path.display()))?;
    Ok(Some(report))
}

/// Runs the debate again on the model replies of the record in `record_dir`, writes the new run
/// record under `runs_dir` and returns the parts in which it differs from the recorded one.
pub async fn replay(record_dir: &Path, runs_dir: &Path) -> anyhow::Result<Vec<String>> {
    let proposal_path = record_dir.join("proposal.md");
    let proposal = std::fs::read_to_string(&proposal_path).with_context(|| format!("reading {}", proposal_path.display()))?;
    let context_path = record_dir.join("context.md");
    let context = if context_path.exists() {
        Some(std::fs::read_to_string(&context_path).with_context(|| format!("reading {}", context_path.display()))?)
    } else {
        None
    };

    let mut config = Config::load(Some(&record_dir.join("config.toml")))?;
    config.runs_dir = Some(runs_dir.to_path_buf());
    let advocate = load_agent("advocate", &config, None, ADVOCATE_SHAPE)?;
    let adversary = load_agent("adversary", &config, None, ADVERSARY_SHAPE)?;

    let replies = Arc::new(ReplayProvider::from_record(record_dir)?);
    let mut providers: BTreeMap<String, Arc<dyn ModelProvider>> = BTreeMap::new();
    for def in [&advocate, &adversary] {
        providers.insert(def.model.provider.clone(), replies.clone());
    }
    let env = Environment { config, advocate, adversary, providers };

    let replayed = match cross_examine(&env, proposal, context, None, CancellationToken::new()).await {
        Ok(output) => output.report.record_dir,
        Err(RunFailure::Failed { record_dir: Some(dir), .. }) => dir,
        Err(RunFailure::Failed { reason, record_dir: None, .. }) => {
            bail!("the replay created no run record: {reason}")
        }
        Err(RunFailure::Cancelled { .. }) => bail!("the replay was cancelled"),
    };

    let mut differing = compare_records(record_dir, &replayed)?;
    let unused = replies.remaining();
    if unused > 0 {
        differing.push(format!("{unused} recorded replies were not used"));
    }
    Ok(differing)
}
