use std::sync::Arc;

use cross_examine::actor::spawn_agent;
use cross_examine::agents::AgentDef;
use cross_examine::debate::{DebateInput, run_debate};
use cross_examine::model::{CritiqueStatus, DebateOutcome, Ending};
use cross_examine::provider::{ModelProvider, ScriptedProvider};
use tokio_util::sync::CancellationToken;

const PROPOSAL: &str = "A is true. B is true. So C is true.";

const TREE: &str = r#"{"claims": [
    {"id": "C1", "text": "A", "places": ["A is true"]},
    {"id": "C2", "text": "B", "places": ["B is true"]},
    {"id": "C3", "text": "C", "places": ["C is true"]}],
  "inferences": [{"id": "I1", "premises": ["C1", "C2"], "conclusion": "C3", "places": ["So"]}],
  "conclusions": ["C3"]}"#;

const TREE_WITH_C9: &str = r#"{"claims": [
    {"id": "C1", "text": "A", "places": ["A is true"]},
    {"id": "C2", "text": "B", "places": ["B is true"]},
    {"id": "C9", "text": "hidden", "places": [], "origin": {"round": 2, "critique": "K2"}},
    {"id": "C3", "text": "C", "places": ["C is true"]}],
  "inferences": [{"id": "I1", "premises": ["C1", "C2", "C9"], "conclusion": "C3", "places": ["So"]}],
  "conclusions": ["C3"]}"#;

fn def(name: &str) -> AgentDef {
    AgentDef {
        name: name.into(),
        provider: "scripted".into(),
        model: name.into(),
        max_tokens: 1000,
        request: Default::default(),
        prompt: "test".into(),
        source: "test".into(),
    }
}

fn gap(id: &str) -> String {
    format!(r#"{{"id": "{id}", "kind": "gap", "inference": "I1", "point": "misses Q", "support": "A and B hold, Q fails, C fails"}}"#)
}

async fn debate(provider: &Arc<ScriptedProvider>, round_limit: u32) -> DebateOutcome {
    let dynamic: Arc<dyn ModelProvider> = provider.clone();
    let advocate = spawn_agent(def("advocate"), dynamic.clone());
    let adversary = spawn_agent(def("adversary"), dynamic);
    let input = DebateInput { proposal: PROPOSAL.into(), context: None, round_limit };
    run_debate(&input, &advocate, &adversary, None, &CancellationToken::new())
        .await
        .unwrap_or_else(|e| panic!("debate failed: {}", e.reason))
}

fn ids<T>(items: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
    items.iter().map(|i| id(i).to_string()).collect()
}

fn status(outcome: &DebateOutcome, id: &str) -> CritiqueStatus {
    outcome.ledger.iter().find(|r| r.critique.id == id).unwrap_or_else(|| panic!("{id} not in ledger")).status
}

#[tokio::test]
async fn satisfied_in_round_one_stops_early() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", r#"{"satisfied": true}"#);
    let out = debate(&p, 3).await;
    assert_eq!(out.ending, Ending::Satisfied { round: 1 });
    assert_eq!(out.rounds_run, 1);
    assert!(out.ledger.is_empty());
    assert_eq!(p.calls(), vec!["advocate", "adversary"]);
}

#[tokio::test]
async fn satisfied_in_round_two() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}], "satisfied": false}}"#, gap("K1")));
    p.push(
        "advocate",
        format!(
            r#"{{"answers": [{{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "stated", "quotes": ["A is true"]}}], "tree": {TREE}}}"#
        ),
    );
    p.push("adversary", r#"{"responses": [{"critique": "K1", "action": "drop"}], "satisfied": true}"#);
    let out = debate(&p, 3).await;
    assert_eq!(out.ending, Ending::Satisfied { round: 2 });
    assert!(out.open.is_empty());
    assert!(out.unanswered.is_empty());
    assert_eq!(status(&out, "K1"), CritiqueStatus::DroppedAfterRejection);
}

#[tokio::test]
async fn limit_splits_open_and_unanswered() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}, {}], "satisfied": false}}"#, gap("K1"), gap("K2")));
    p.push(
        "advocate",
        format!(
            r#"{{"answers": [
                {{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "stated", "quotes": ["A is true"]}},
                {{"critique": "K2", "kind": "accept", "reasons": "added C9"}}],
              "tree": {TREE_WITH_C9}}}"#
        ),
    );
    p.push(
        "adversary",
        format!(
            r#"{{"responses": [{{"critique": "K1", "action": "maintain", "reasons": "the quote does not state Q"}}],
                "critiques": [{}], "satisfied": false}}"#,
            gap("K3")
        ),
    );
    let out = debate(&p, 2).await;
    assert_eq!(out.ending, Ending::LimitReached);
    assert_eq!(out.rounds_run, 2);
    assert_eq!(ids(&out.open, |o| &o.critique.id), vec!["K1"]);
    assert_eq!(ids(&out.unanswered, |c| &c.id), vec!["K3"]);
    assert_eq!(ids(&out.changes, |c| &c.critique.id), vec!["K2"]);
    assert_eq!(status(&out, "K1"), CritiqueStatus::Open);
    assert_eq!(status(&out, "K2"), CritiqueStatus::Accepted);
    assert_eq!(status(&out, "K3"), CritiqueStatus::Unanswered);
    assert_eq!(out.ledger.iter().find(|r| r.critique.id == "K3").unwrap().raised_in, 2);
}

#[tokio::test]
async fn cancelled_before_start_calls_no_model() {
    for _ in 0..40 {
        let p = Arc::new(ScriptedProvider::new());
        p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
        let dynamic: Arc<dyn ModelProvider> = p.clone();
        let advocate = spawn_agent(def("advocate"), dynamic.clone());
        let adversary = spawn_agent(def("adversary"), dynamic);
        let input = DebateInput { proposal: PROPOSAL.into(), context: None, round_limit: 2 };
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = run_debate(&input, &advocate, &adversary, None, &cancel).await.expect_err("cancelled");
        assert!(err.cancelled);
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert!(p.calls().is_empty(), "a model call started after cancellation");
    }
}

#[tokio::test]
async fn limit_one_leaves_every_critique_unanswered() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}], "satisfied": false}}"#, gap("K1")));
    let out = debate(&p, 1).await;
    assert_eq!(out.ending, Ending::LimitReached);
    assert!(out.open.is_empty());
    assert_eq!(ids(&out.unanswered, |c| &c.id), vec!["K1"]);
    assert_eq!(status(&out, "K1"), CritiqueStatus::Unanswered);
}
