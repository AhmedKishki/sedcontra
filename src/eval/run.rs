//! Runs every case of an evaluation several times through the cross-examination, scores the runs and writes the scores.

use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::eval::case::Case;
use crate::eval::score::{Outcome, RunScore, score};
use crate::event::{LogLine, total_usage};
use crate::model::{CritiqueStatus, Ending};
use crate::provider::Usage;
use crate::record::{COMMIT, RunRecord};
use crate::run::{Environment, RunFailure, cross_examine};

#[derive(Serialize, Clone, Debug)]
pub struct RunResult {
    pub case: String,
    pub repeat: u32,
    pub run_id: Option<String>,
    pub failure: Option<String>,
    pub ending: Option<Ending>,
    pub rounds_run: Option<u32>,
    pub critiques: usize,
    pub score: Option<RunScore>,
    pub rejected: usize,
    pub calls: usize,
    pub usage: Usage,
}

#[derive(Serialize, Clone, Debug)]
pub struct DefectSummary {
    pub case: String,
    pub defect: String,
    pub kind: String,
    pub runs: u32,
    pub missed: u32,
    pub accepted: u32,
    pub open: u32,
    pub dropped: u32,
    pub same_kind: u32,
}

#[derive(Serialize, Clone, Debug)]
pub struct CaseSummary {
    pub case: String,
    pub runs: u32,
    pub failed: u32,
    pub critiques: usize,
    pub unplanted_accepted: usize,
    pub unplanted_open: usize,
    pub unplanted_dropped: usize,
    pub rejected: usize,
    pub calls: usize,
    pub usage: Usage,
}

#[derive(Serialize, Clone, Debug)]
pub struct Scores {
    pub commit: String,
    pub cases_dir: String,
    pub repeat: u32,
    pub round_limit: u32,
    pub defects: Vec<DefectSummary>,
    pub cases: Vec<CaseSummary>,
    pub runs: Vec<RunResult>,
}

/// Runs every case `eval.repeat` times, at most `eval.jobs` runs at once, writes `scores.json` and
/// `scores.md` into `out` and returns the scores. The run folders go under `out/runs`.
pub async fn evaluate(
    mut env: Environment,
    cases: Vec<Case>,
    cases_dir: &Path,
    out: &Path,
    cancel: CancellationToken,
) -> anyhow::Result<Scores> {
    let (repeat, jobs) = (env.config.eval.repeat, env.config.eval.jobs);
    if repeat == 0 || jobs == 0 {
        bail!("eval.repeat and eval.jobs must be at least 1");
    }
    env.config.runs_dir = Some(out.join("runs"));
    let round_limit = env.config.round_limit();
    let env = Arc::new(env);
    // `Semaphore::new` panics above its maximum, and a mistyped `jobs` can exceed it.
    let slots = Arc::new(Semaphore::new(jobs.min(Semaphore::MAX_PERMITS)));
    // A run that fails to leave a result stops the others through `stop`, while `cancel` stays the caller's.
    let stop = cancel.child_token();

    let mut tasks = JoinSet::new();
    for (index, case) in cases.iter().enumerate() {
        for number in 1..=repeat {
            let (env, case, slots, stop) = (env.clone(), case.clone(), slots.clone(), stop.clone());
            tasks.spawn(async move {
                let _slot = tokio::select! {
                    biased;
                    _ = stop.cancelled() => return Err(anyhow!("cancelled")),
                    slot = slots.acquire_owned() => slot.context("the run slots of the evaluation were closed")?,
                };
                let result = run_case(&env, &case, number, repeat, stop).await?;
                Ok((index, result))
            });
        }
    }

    let mut finished = Vec::new();
    let mut failure = None;
    while let Some(joined) = tasks.join_next().await {
        let result = joined.unwrap_or_else(|e| Err(anyhow!("a run of the evaluation ended without a result: {e}")));
        match result {
            Ok(run) => finished.push(run),
            Err(e) => {
                stop.cancel();
                failure.get_or_insert(e);
            }
        }
    }
    if let Some(failure) = failure {
        return Err(if cancel.is_cancelled() { anyhow!("cancelled") } else { failure });
    }

    finished.sort_by_key(|(index, run)| (*index, run.repeat));
    let mut defects = Vec::new();
    let mut summaries = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let runs: Vec<&RunResult> = finished.iter().filter(|(i, _)| *i == index).map(|(_, run)| run).collect();
        defects.extend(defect_rows(case, &runs));
        summaries.push(case_row(case, &runs));
    }
    let scores = Scores {
        commit: COMMIT.to_string(),
        cases_dir: cases_dir.display().to_string(),
        repeat,
        round_limit,
        defects,
        cases: summaries,
        runs: finished.into_iter().map(|(_, run)| run).collect(),
    };

    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let json_path = out.join("scores.json");
    let json = serde_json::to_string_pretty(&scores).context("serialising the scores")?;
    std::fs::write(&json_path, json).with_context(|| format!("writing {}", json_path.display()))?;
    let markdown_path = out.join("scores.md");
    std::fs::write(&markdown_path, render_scores(&scores)).with_context(|| format!("writing {}", markdown_path.display()))?;
    Ok(scores)
}

/// One run of `case`, number `number` of `total`, scored when it finished. Only a cancelled run and
/// a record folder whose log cannot be read are errors: a failed debate is a result.
async fn run_case(env: &Environment, case: &Case, number: u32, total: u32, cancel: CancellationToken) -> anyhow::Result<RunResult> {
    let mut result = RunResult {
        case: case.name.clone(),
        repeat: number,
        run_id: None,
        failure: None,
        ending: None,
        rounds_run: None,
        critiques: 0,
        score: None,
        rejected: 0,
        calls: 0,
        usage: Usage::new(),
    };
    let record_dir = match cross_examine(env, case.proposal.clone(), case.context.clone(), None, cancel).await {
        Ok(output) => {
            let report = output.report;
            result.critiques = report.threads.len();
            result.score = Some(score(case, &report.threads));
            result.run_id = Some(report.run_id);
            result.ending = Some(report.ending);
            result.rounds_run = Some(report.rounds_run);
            Some(report.record_dir)
        }
        Err(RunFailure::Failed { reason, record_dir, .. }) => {
            result.run_id = record_dir.as_deref().and_then(Path::file_name).map(|name| name.to_string_lossy().into_owned());
            result.failure = Some(reason);
            record_dir
        }
        Err(RunFailure::Cancelled { .. }) => bail!("cancelled"),
    };

    let log = match record_dir.as_deref() {
        Some(dir) if RunRecord::has_log(dir) => RunRecord::read_log(dir)?,
        _ => Vec::new(),
    };
    result.calls = log.iter().filter(|line| matches!(line, LogLine::Call(_))).count();
    result.rejected = log.iter().filter(|line| matches!(line, LogLine::Rejected(_))).count();
    result.usage = total_usage(&log);

    let state = if result.failure.is_none() { "done" } else { "failed" };
    eprintln!("{} {number}/{total}: {state}", case.name);
    Ok(result)
}

fn defect_rows(case: &Case, runs: &[&RunResult]) -> Vec<DefectSummary> {
    case.defects
        .iter()
        .map(|defect| {
            let mut row = DefectSummary {
                case: case.name.clone(),
                defect: defect.id.clone(),
                kind: defect.kind.clone(),
                runs: 0,
                missed: 0,
                accepted: 0,
                open: 0,
                dropped: 0,
                same_kind: 0,
            };
            let scored = runs
                .iter()
                .filter_map(|run| run.score.as_ref())
                .filter_map(|score| score.defects.iter().find(|scored| scored.defect == defect.id));
            for scored in scored {
                row.runs += 1;
                match scored.outcome {
                    Outcome::Missed => row.missed += 1,
                    Outcome::Accepted => row.accepted += 1,
                    Outcome::Open => row.open += 1,
                    Outcome::Dropped => row.dropped += 1,
                }
                if scored.same_kind {
                    row.same_kind += 1;
                }
            }
            row
        })
        .collect()
}

fn case_row(case: &Case, runs: &[&RunResult]) -> CaseSummary {
    let mut row = CaseSummary {
        case: case.name.clone(),
        runs: 0,
        failed: 0,
        critiques: 0,
        unplanted_accepted: 0,
        unplanted_open: 0,
        unplanted_dropped: 0,
        rejected: 0,
        calls: 0,
        usage: Usage::new(),
    };
    for run in runs {
        row.runs += 1;
        if run.failure.is_some() {
            row.failed += 1;
        }
        row.critiques += run.critiques;
        row.rejected += run.rejected;
        row.calls += run.calls;
        add_usage(&mut row.usage, &run.usage);
        for landing in run.score.iter().flat_map(|score| &score.unplanted) {
            match landing.status {
                CritiqueStatus::Accepted => row.unplanted_accepted += 1,
                CritiqueStatus::Open => row.unplanted_open += 1,
                CritiqueStatus::Dropped => row.unplanted_dropped += 1,
            }
        }
    }
    row
}

fn add_usage(total: &mut Usage, usage: &Usage) {
    for (key, value) in usage {
        let Some(count) = value.as_u64() else { continue };
        let sum = total.get(key).and_then(Value::as_u64).unwrap_or(0).saturating_add(count);
        total.insert(key.clone(), Value::from(sum));
    }
}

pub fn render_scores(scores: &Scores) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "# Evaluation scores\n");
    let _ = writeln!(
        out,
        "Commit `{}`, cases `{}`, {} runs per case, round limit {}.\n",
        scores.commit, scores.cases_dir, scores.repeat, scores.round_limit
    );

    let _ = writeln!(out, "## Planted defects\n");
    if scores.defects.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(
            out,
            "| Case | Defect | Kind | Runs | Missed | Accepted | Open | Dropped | Same kind |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
        );
        for row in &scores.defects {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                row.case, row.defect, row.kind, row.runs, row.missed, row.accepted, row.open, row.dropped, row.same_kind
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Cases\n");
    if scores.cases.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(
            out,
            "| Case | Runs | Failed | Critiques | Unplanted accepted | Unplanted open | Unplanted dropped | Rejected replies | Calls | Usage |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |"
        );
        for row in &scores.cases {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                row.case,
                row.runs,
                row.failed,
                row.critiques,
                row.unplanted_accepted,
                row.unplanted_open,
                row.unplanted_dropped,
                row.rejected,
                row.calls,
                usage_cell(&row.usage)
            );
        }
        out.push('\n');
    }

    let _ = writeln!(
        out,
        "A critique that lands on no planted defect is counted above and judged nowhere: whether it is right needs a reader."
    );
    out
}

fn usage_cell(usage: &Usage) -> String {
    if usage.is_empty() {
        return "-".to_string();
    }
    usage.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join(", ")
}

pub fn render_cases(cases: &[Case]) -> String {
    let mut out = String::new();
    for case in cases {
        let _ = writeln!(out, "{}: {} defects", case.name, case.defects.len());
        for defect in &case.defects {
            let place = defect.places.first().map(|place| place.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
            let _ = writeln!(out, "  {} {}: \"{place}\"", defect.id, defect.kind);
        }
    }
    out
}
