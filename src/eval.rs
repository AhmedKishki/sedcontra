//! Scores the tool against the Arguinas test split with a judge agent.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::actor::{AgentHandle, spawn_agent};
use crate::agents::{AgentDef, load_agent};
use crate::model::{ClaimId, CritiqueId, Ending, Turn};
use crate::provider::ModelProvider;
use crate::report::Report;
use crate::run::{Environment, RunFailure, cross_examine};

/// The dataset fields the evaluation reads.
#[derive(Deserialize, Clone)]
pub struct EvalRecord {
    pub title: String,
    pub background: String,
    pub argument: String,
    pub explicit_premises: Vec<String>,
    pub implicit_premises: Vec<String>,
    pub fallacy_info: Option<FallacyInfo>,
    pub sections: Sections,
}

#[derive(Deserialize, Clone)]
pub struct FallacyInfo {
    pub rationale: String,
}

#[derive(Deserialize, Clone)]
pub struct Sections {
    pub reconstruction: Reconstruction,
}

#[derive(Deserialize, Clone)]
pub struct Reconstruction {
    pub conclusion: String,
}

/// The judge's matches. The reference statements are the conclusion `K`, the explicit premises
/// `E1…` and the implicit premises `H1…`; for each, the claim of each tree that states it.
#[derive(Serialize, Deserialize, Debug)]
pub struct Judgement {
    #[serde(default)]
    pub first_tree: BTreeMap<String, Option<ClaimId>>,
    #[serde(default)]
    pub final_tree: BTreeMap<String, Option<ClaimId>>,
    #[serde(default)]
    pub fallacy: Option<CritiqueId>,
}

#[derive(Serialize, Default, Debug)]
pub struct RecordScore {
    pub index: usize,
    pub run_id: Option<String>,
    pub error: Option<String>,
    pub ending: Option<Ending>,
    pub rounds_run: Option<u32>,
    pub conclusion_in_first: Option<bool>,
    pub conclusion_in_final: Option<bool>,
    /// The final tree states the reference conclusion in a claim it marks as a conclusion.
    pub conclusion_marked: Option<bool>,
    /// The final tree lacks the reference conclusion, but a conclusion it marks states a reference premise.
    pub stopped_short: Option<bool>,
    pub explicit_in_first: Option<f64>,
    pub explicit_in_final: Option<f64>,
    pub implicit_in_first: Option<f64>,
    pub implicit_in_final: Option<f64>,
    pub fallacy_identified: Option<bool>,
    /// The judge's matches, kept so a person can audit them against the run folder's trees.
    pub judgement: Option<Judgement>,
}

pub async fn run_eval(
    mut env: Environment,
    dataset: &Path,
    limit: Option<usize>,
    concurrency: usize,
    out: &Path,
    cancel: CancellationToken,
) -> anyhow::Result<Vec<RecordScore>> {
    let text = std::fs::read_to_string(dataset).with_context(|| format!("reading {}", dataset.display()))?;
    let mut records: Vec<EvalRecord> =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", dataset.display()))?;
    if let Some(limit) = limit {
        records.truncate(limit);
    }

    let judge = load_agent("judge", &env.config)?;
    let judge_provider = env.provider_for(&judge)?;
    let env = Arc::new(env);

    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = std::fs::File::create(out).with_context(|| format!("creating {}", out.display()))?;

    let gate = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut tasks = JoinSet::new();
    for (index, record) in records.into_iter().enumerate() {
        let (env, judge, judge_provider, gate, cancel) =
            (env.clone(), judge.clone(), judge_provider.clone(), gate.clone(), cancel.clone());
        tasks.spawn(async move {
            let _permit = gate.acquire_owned().await;
            score_record(&env, judge, judge_provider, index, record, cancel).await
        });
    }

    let mut scores = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        let score = joined.unwrap_or_else(|e| RecordScore { error: Some(e.to_string()), ..Default::default() });
        eprintln!("record {}: {}", score.index, score.error.as_deref().unwrap_or("scored"));
        writeln!(file, "{}", serde_json::to_string(&score)?)?;
        scores.push(score);
    }
    scores.sort_by_key(|s| s.index);
    Ok(scores)
}

async fn score_record(
    env: &Environment,
    judge: AgentDef,
    judge_provider: Arc<dyn ModelProvider>,
    index: usize,
    record: EvalRecord,
    cancel: CancellationToken,
) -> RecordScore {
    let failed = |error: String, run_id: Option<String>| RecordScore { index, run_id, error: Some(error), ..Default::default() };
    // A record still queued when the evaluation is cancelled creates no run.
    if cancel.is_cancelled() {
        return failed("cancelled".into(), None);
    }
    let context = match record.background.trim() {
        "" | "None" => format!("Debate topic: {}", record.title),
        background => format!("Debate topic: {}\n\n{background}", record.title),
    };
    let output = match cross_examine(env, record.argument.clone(), Some(context), None, cancel.clone()).await {
        Ok(output) => output,
        Err(RunFailure::Failed { reason, .. }) => return failed(reason, None),
        Err(RunFailure::Cancelled { .. }) => return failed("cancelled".into(), None),
    };
    let report = output.report;
    let lists = Lists::from_report(&report);

    let handle = spawn_agent(judge, judge_provider);
    let judgement = match ask_judge(&handle, &record, &report, &lists, &cancel).await {
        Ok(judgement) => judgement,
        Err(error) => return failed(error, Some(report.run_id.clone())),
    };

    let found = |matches: &BTreeMap<String, Option<ClaimId>>, key: &str| matches.get(key).is_some_and(Option::is_some);
    // Only the expected keys count, so a stray key can never raise a rate.
    let rate = |matches: &BTreeMap<String, Option<ClaimId>>, prefix: &str, total: usize| {
        let found = (1..=total).filter(|i| matches.get(&format!("{prefix}{i}")).is_some_and(Option::is_some)).count();
        (total > 0).then(|| found as f64 / total as f64)
    };
    let (first, last) = (&judgement.first_tree, &judgement.final_tree);
    let conclusion_marked = last.get("K").cloned().flatten().is_some_and(|id| lists.marked.contains(&id));
    let stopped_short = !found(last, "K")
        && last.iter().any(|(key, id)| key != "K" && id.as_ref().is_some_and(|id| lists.marked.contains(id)));
    RecordScore {
        index,
        run_id: Some(report.run_id.clone()),
        error: None,
        ending: Some(report.ending.clone()),
        rounds_run: Some(report.rounds_run),
        conclusion_in_first: Some(found(first, "K")),
        conclusion_in_final: Some(found(last, "K")),
        conclusion_marked: Some(conclusion_marked),
        stopped_short: Some(stopped_short),
        explicit_in_first: rate(first, "E", record.explicit_premises.len()),
        explicit_in_final: rate(last, "E", record.explicit_premises.len()),
        implicit_in_first: rate(first, "H", record.implicit_premises.len()),
        implicit_in_final: rate(last, "H", record.implicit_premises.len()),
        fallacy_identified: record.fallacy_info.as_ref().map(|_| judgement.fallacy.is_some()),
        judgement: Some(judgement),
    }
}

/// Our side of the comparison, each as (id, text), and the conclusions the final tree marks.
struct Lists {
    first_tree: Vec<(String, String)>,
    final_tree: Vec<(String, String)>,
    marked: BTreeSet<ClaimId>,
    critiques: Vec<(String, String)>,
}

impl Lists {
    fn from_report(report: &Report) -> Self {
        let tree = &report.final_tree.tree;
        let first_tree = report
            .transcript
            .iter()
            .find_map(|t| match t {
                Turn::Advocate { round: 1, turn } => {
                    Some(turn.tree.claims.iter().map(|c| (c.id.clone(), c.text.clone())).collect())
                }
                _ => None,
            })
            .unwrap_or_default();
        Lists {
            first_tree,
            final_tree: tree.claims.iter().map(|c| (c.id.clone(), c.text.clone())).collect(),
            marked: tree.conclusions.iter().cloned().collect(),
            critiques: report
                .ledger
                .iter()
                .map(|row| {
                    let target = &row.critique.target;
                    (row.critique.id.clone(), format!("({} on {}) {}", target.kind(), target.describe(), row.critique.point))
                })
                .collect(),
        }
    }
}

async fn ask_judge(
    handle: &AgentHandle,
    record: &EvalRecord,
    report: &Report,
    lists: &Lists,
    cancel: &CancellationToken,
) -> Result<Judgement, String> {
    let mut message = judge_message(record, lists);
    for attempt in 0..2 {
        let reply = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err("cancelled".into()),
            reply = handle.turn(message) => reply.map_err(|e| format!("judge: {e} (run {})", report.run_id))?,
        };
        let errors = match serde_json::from_value::<Judgement>(reply.value) {
            Ok(judgement) => match check_judgement(&judgement, record, lists) {
                Ok(()) => return Ok(judgement),
                Err(errors) => errors,
            },
            Err(e) => vec![format!("the reply does not have the required JSON shape: {e}")],
        };
        if attempt == 1 {
            return Err(format!("judge failed the checks twice: {}", errors.join("; ")));
        }
        message = format!(
            "Your reply failed these checks:\n{}\n\nReply again with the whole corrected JSON object.",
            errors.iter().map(|e| format!("- {e}")).collect::<Vec<_>>().join("\n")
        );
    }
    unreachable!("the loop returns on its second attempt")
}

fn judge_message(record: &EvalRecord, lists: &Lists) -> String {
    let mut out = String::new();
    let section = |out: &mut String, heading: &str, items: Vec<(String, String)>| {
        let _ = writeln!(out, "## {heading}\n");
        if items.is_empty() {
            let _ = writeln!(out, "None.");
        }
        for (id, text) in items {
            let _ = writeln!(out, "- {id}: {}", text.split_whitespace().collect::<Vec<_>>().join(" "));
        }
        out.push('\n');
    };
    let labelled = |prefix: &str, items: &[String]| -> Vec<(String, String)> {
        items.iter().enumerate().map(|(i, t)| (format!("{prefix}{}", i + 1), t.clone())).collect()
    };

    section(&mut out, "Reference conclusion", vec![("K".into(), record.sections.reconstruction.conclusion.clone())]);
    section(&mut out, "Reference explicit premises", labelled("E", &record.explicit_premises));
    section(&mut out, "Reference implicit premises", labelled("H", &record.implicit_premises));
    if let Some(fallacy) = &record.fallacy_info {
        let _ = writeln!(out, "## Reference fallacy rationale\n\n{}\n", fallacy.rationale.trim());
    }
    section(&mut out, "Claims of the first tree", lists.first_tree.clone());
    section(&mut out, "Claims of the final tree", lists.final_tree.clone());
    section(&mut out, "Critiques", lists.critiques.clone());
    out.push_str("Answer with the JSON object your instructions describe.");
    out
}

fn check_judgement(judgement: &Judgement, record: &EvalRecord, lists: &Lists) -> Result<(), Vec<String>> {
    let ids = |list: &[(String, String)]| -> BTreeSet<String> { list.iter().map(|(id, _)| id.clone()).collect() };
    let (first, last, critiques) = (ids(&lists.first_tree), ids(&lists.final_tree), ids(&lists.critiques));
    let expected: BTreeSet<String> = std::iter::once("K".to_string())
        .chain((1..=record.explicit_premises.len()).map(|i| format!("E{i}")))
        .chain((1..=record.implicit_premises.len()).map(|i| format!("H{i}")))
        .collect();
    let mut errors = Vec::new();

    for (name, matches, allowed) in [("first_tree", &judgement.first_tree, &first), ("final_tree", &judgement.final_tree, &last)] {
        for key in matches.keys().filter(|k| !expected.contains(*k)) {
            errors.push(format!("{name} has the key {key}, which names no reference statement"));
        }
        for key in &expected {
            match matches.get(key) {
                None => errors.push(format!("{name} has no entry for {key}")),
                Some(Some(id)) if !allowed.contains(id) => {
                    errors.push(format!("{name}.{key} names {id}, which is not a claim of that tree"))
                }
                _ => {}
            }
        }
    }

    if let Some(id) = &judgement.fallacy
        && !critiques.contains(id)
    {
        errors.push(format!("fallacy names {id}, which is not a critique"));
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

pub fn summarize(scores: &[RecordScore]) -> String {
    let mean_bool = |f: fn(&RecordScore) -> Option<bool>| -> (Option<f64>, usize) {
        let values: Vec<bool> = scores.iter().filter_map(f).collect();
        let mean = (!values.is_empty()).then(|| values.iter().filter(|v| **v).count() as f64 / values.len() as f64);
        (mean, values.len())
    };
    let mean_rate = |f: fn(&RecordScore) -> Option<f64>| -> (Option<f64>, usize) {
        let values: Vec<f64> = scores.iter().filter_map(f).collect();
        let mean = (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64);
        (mean, values.len())
    };
    let cell = |mean: Option<f64>| mean.map(|m| format!("{m:.2}")).unwrap_or_else(|| "–".into());
    let rows = [
        ("Conclusion", mean_bool(|s| s.conclusion_in_first), mean_bool(|s| s.conclusion_in_final)),
        ("Explicit premises", mean_rate(|s| s.explicit_in_first), mean_rate(|s| s.explicit_in_final)),
        ("Implicit premises", mean_rate(|s| s.implicit_in_first), mean_rate(|s| s.implicit_in_final)),
    ];

    let mut out = String::from(
        "The share of the reference reconstruction that each tree states:\n\n\
         | Reference part | First tree | Final tree | Records |\n| --- | --- | --- | --- |\n",
    );
    for (name, first, last) in rows {
        let _ = writeln!(out, "| {name} | {} | {} | {} |", cell(first.0), cell(last.0), last.1);
    }
    out.push_str(
        "\nThe first tree states what the text says, so a low share of implicit premises there is intended; \
         the rise from the first tree to the final tree is what the debate added.\n",
    );

    let absent = scores.iter().filter(|s| s.conclusion_in_final == Some(false)).count();
    let short = scores.iter().filter(|s| s.stopped_short == Some(true)).count();
    let (marked, scored) = mean_bool(|s| s.conclusion_marked);
    let (fallacy, labelled) = mean_bool(|s| s.fallacy_identified);
    let _ = writeln!(
        out,
        "\nThe final tree marks the reference conclusion as a conclusion in {} of {scored} records. \
         Of the {absent} records whose final tree lacks it, {short} end on a claim that the reference uses as a premise.",
        marked.map(|m| (m * scored as f64).round() as usize).unwrap_or(0),
    );
    let _ = writeln!(
        out,
        "Fallacy identified: {} of {labelled} records with a fallacy label.",
        fallacy.map(|m| (m * labelled as f64).round() as usize).unwrap_or(0),
    );

    let satisfied = scores.iter().filter(|s| matches!(s.ending, Some(Ending::Satisfied { .. }))).count();
    let limit = scores.iter().filter(|s| matches!(s.ending, Some(Ending::LimitReached))).count();
    let failed = scores.iter().filter(|s| s.error.is_some()).count();
    let _ = writeln!(
        out,
        "\nEndings: {satisfied} satisfied, {limit} at the round limit, {failed} failed, of {} records.",
        scores.len()
    );
    out
}
