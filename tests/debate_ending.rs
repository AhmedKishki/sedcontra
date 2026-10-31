use std::sync::Arc;

use sedcontra::actor::spawn_agent;
use sedcontra::agents::AgentDef;
use sedcontra::config::{CheckConfig, ModelSettings};
use sedcontra::debate::{DebateInput, run_debate};
use sedcontra::model::{CritiqueStatus, DebateOutcome, Effect, Ending};
use sedcontra::provider::{ModelProvider, ScriptedProvider};
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
        prompt: "test".into(),
        source: "test".into(),
        model: ModelSettings { provider: "scripted".into(), model: name.into(), max_tokens: 1000, request: Default::default() },
    }
}

fn gap(id: &str) -> String {
    format!(r#"{{"id": "{id}", "kind": "gap", "inference": "I1", "point": "misses Q", "support": "A and B hold, Q fails, C fails"}}"#)
}

async fn debate(provider: &Arc<ScriptedProvider>, round_limit: u32) -> DebateOutcome {
    let dynamic: Arc<dyn ModelProvider> = provider.clone();
    let advocate = spawn_agent(def("advocate"), dynamic.clone());
    let adversary = spawn_agent(def("adversary"), dynamic);
    let input = DebateInput { proposal: PROPOSAL.into(), context: None, round_limit, retries: 1, checks: CheckConfig::bundled() };
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
            r#"{{"answers": [{{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "stated", "quotes": ["A is true"], "premises": ["C1"]}}], "tree": {TREE}}}"#
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
                {{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "stated", "quotes": ["A is true"], "premises": ["C1"]}},
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
    assert_eq!(out.changes[0].effects, vec![Effect::AddedClaim { claim: "C9".into() }]);
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
        let input = DebateInput { proposal: PROPOSAL.into(), context: None, round_limit: 2, retries: 1, checks: CheckConfig::bundled() };
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

const CONCEDE_K1: &str = r#"{"critique": "K1", "kind": "concede", "reasons": "no premise the proposal could hold closes the gap"}"#;

#[tokio::test]
async fn conceded_critique_ends_conceded() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}], "satisfied": false}}"#, gap("K1")));
    p.push("advocate", format!(r#"{{"answers": [{CONCEDE_K1}], "tree": {TREE}}}"#));
    p.push("adversary", r#"{"satisfied": true}"#);
    let out = debate(&p, 3).await;
    assert_eq!(out.ending, Ending::Satisfied { round: 2 });
    assert_eq!(status(&out, "K1"), CritiqueStatus::Conceded);
}

#[tokio::test]
async fn concession_at_round_limit_stays_conceded() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}, {}], "satisfied": false}}"#, gap("K1"), gap("K2")));
    p.push(
        "advocate",
        format!(
            r#"{{"answers": [{CONCEDE_K1},
                {{"critique": "K2", "kind": "reject", "ground": "stated", "reasons": "stated", "quotes": ["A is true"], "premises": ["C1"]}}],
              "tree": {TREE}}}"#
        ),
    );
    p.push(
        "adversary",
        r#"{"responses": [{"critique": "K2", "action": "maintain", "reasons": "the quote does not state Q"}],
            "critiques": [{"id": "K3", "kind": "reading", "of": "C3", "point": "C is hedged", "support": "the text says C is true", "quotes": ["C is true"]}],
            "satisfied": false}"#,
    );
    let out = debate(&p, 2).await;
    assert_eq!(out.ending, Ending::LimitReached);
    assert_eq!(status(&out, "K1"), CritiqueStatus::Conceded);
    assert_eq!(status(&out, "K2"), CritiqueStatus::Open);
    assert_eq!(status(&out, "K3"), CritiqueStatus::Unanswered);
    assert_eq!(ids(&out.open, |o| &o.critique.id), vec!["K2"]);
    assert_eq!(ids(&out.unanswered, |c| &c.id), vec!["K3"]);
}

#[tokio::test]
async fn gap_on_conceded_inference_is_refused_and_retried() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}], "satisfied": false}}"#, gap("K1")));
    p.push("advocate", format!(r#"{{"answers": [{CONCEDE_K1}], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}], "satisfied": false}}"#, gap("K2")));
    p.push("adversary", r#"{"satisfied": true}"#);
    let out = debate(&p, 3).await;
    assert_eq!(out.ending, Ending::Satisfied { round: 2 });
    assert_eq!(p.calls(), vec!["advocate", "adversary", "advocate", "adversary", "adversary"]);
}

#[tokio::test]
async fn conceded_and_closed_lists() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}, {}], "satisfied": false}}"#, gap("K1"), gap("K2")));
    p.push(
        "advocate",
        format!(
            r#"{{"answers": [{CONCEDE_K1},
                {{"critique": "K2", "kind": "reject", "ground": "stated", "reasons": "stated", "quotes": ["A is true"], "premises": ["C1"]}}],
              "tree": {TREE}}}"#
        ),
    );
    p.push("adversary", r#"{"responses": [{"critique": "K2", "action": "drop"}], "satisfied": true}"#);
    let out = debate(&p, 3).await;
    assert_eq!(ids(&out.conceded, |c| &c.critique.id), vec!["K1"]);
    assert_eq!(ids(&out.closed, |c| &c.critique.id), vec!["K2"]);
}

#[tokio::test]
async fn closed_needs_the_drop_that_answered_the_stated_rejection() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", format!(r#"{{"critiques": [{}], "satisfied": false}}"#, gap("K1")));
    p.push(
        "advocate",
        format!(
            r#"{{"answers": [{{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "stated", "quotes": ["A is true"], "premises": ["C1"]}}], "tree": {TREE}}}"#
        ),
    );
    p.push(
        "adversary",
        r#"{"responses": [{"critique": "K1", "action": "maintain", "reasons": "A does not state Q"}], "satisfied": false}"#,
    );
    p.push(
        "advocate",
        format!(
            r#"{{"answers": [{{"critique": "K1", "kind": "reject", "ground": "counter_fails", "reasons": "Q holds whenever A does"}}], "tree": {TREE}}}"#
        ),
    );
    p.push("adversary", r#"{"responses": [{"critique": "K1", "action": "drop"}], "satisfied": true}"#);
    let out = debate(&p, 3).await;
    assert_eq!(status(&out, "K1"), CritiqueStatus::DroppedAfterRejection);
    assert!(out.closed.is_empty(), "{:?}", ids(&out.closed, |c| &c.critique.id));
}

#[tokio::test]
async fn two_critiques_on_one_inference_split_their_effects() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    let superfluous = r#"{"id": "K2", "kind": "superfluous", "inference": "I1", "premise": "C2", "point": "C2 is idle", "support": "C follows from A"}"#;
    p.push("adversary", format!(r#"{{"critiques": [{}, {superfluous}], "satisfied": false}}"#, gap("K1")));
    p.push(
        "advocate",
        r#"{"answers": [{"critique": "K1", "kind": "accept", "reasons": "added C9"},
                        {"critique": "K2", "kind": "accept", "reasons": "removed C2"}],
            "tree": {"claims": [
                {"id": "C1", "text": "A", "places": ["A is true"]},
                {"id": "C2", "text": "B", "places": ["B is true"]},
                {"id": "C9", "text": "hidden", "places": [], "origin": {"round": 2, "critique": "K1"}},
                {"id": "C3", "text": "C", "places": ["C is true"]}],
              "inferences": [{"id": "I1", "premises": ["C1", "C9"], "conclusion": "C3", "places": ["So"]}],
              "conclusions": ["C3"]}}"#,
    );
    p.push("adversary", r#"{"satisfied": true}"#);
    let out = debate(&p, 3).await;
    let effects = |id: &str| out.changes.iter().find(|c| c.critique.id == id).map(|c| c.effects.clone()).unwrap_or_default();
    assert_eq!(effects("K1"), vec![Effect::AddedClaim { claim: "C9".into() }]);
    assert_eq!(effects("K2"), vec![Effect::RemovedPremise { inference: "I1".into(), premise: "C2".into() }]);
}

/// The adversary's reply that raises and maintains nothing yet is not satisfied, which the checks refuse.
const EMPTY_TURN: &str = r#"{"satisfied": false}"#;

async fn debate_with_retries(provider: &Arc<ScriptedProvider>, retries: u32) -> Result<DebateOutcome, String> {
    let dynamic: Arc<dyn ModelProvider> = provider.clone();
    let advocate = spawn_agent(def("advocate"), dynamic.clone());
    let adversary = spawn_agent(def("adversary"), dynamic);
    let input = DebateInput { proposal: PROPOSAL.into(), context: None, round_limit: 1, retries, checks: CheckConfig::bundled() };
    run_debate(&input, &advocate, &adversary, None, &CancellationToken::new()).await.map_err(|e| e.reason)
}

#[tokio::test]
async fn second_retry_rescues_a_refused_turn() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", EMPTY_TURN);
    p.push("adversary", EMPTY_TURN);
    p.push("adversary", r#"{"satisfied": true}"#);
    let out = debate_with_retries(&p, 2).await.unwrap_or_else(|reason| panic!("debate failed: {reason}"));
    assert_eq!(out.ending, Ending::Satisfied { round: 1 });
    assert_eq!(p.calls(), vec!["advocate", "adversary", "adversary", "adversary"]);
}

#[tokio::test]
async fn turn_fails_after_its_last_retry() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("advocate", format!(r#"{{"answers": [], "tree": {TREE}}}"#));
    p.push("adversary", EMPTY_TURN);
    p.push("adversary", EMPTY_TURN);
    p.push("adversary", r#"{"satisfied": true}"#);
    let reason = debate_with_retries(&p, 1).await.expect_err("the adversary's turn fails after one retry");
    assert!(reason.contains("the adversary's turn failed the checks twice"), "{reason}");
    assert_eq!(p.calls(), vec!["advocate", "adversary", "adversary"]);
}
