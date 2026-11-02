use std::sync::Arc;

use sedcontra::actor::spawn_agent;
use sedcontra::agents::AgentDef;
use sedcontra::config::ModelSettings;
use sedcontra::debate::{DebateError, DebateInput, run_debate};
use sedcontra::model::{AnswerKind, CritiqueStatus, DebateOutcome, Ending, Reply, ResponseAction};
use sedcontra::provider::{ModelProvider, ScriptedProvider};
use tokio_util::sync::CancellationToken;

const PROPOSAL: &str = "A is true. B is true. So C is true.";

fn def(name: &str) -> AgentDef {
    AgentDef {
        name: name.into(),
        prompt: "test".into(),
        source: "test".into(),
        model: ModelSettings { provider: "scripted".into(), model: name.into(), max_tokens: 1000, request: Default::default() },
    }
}

async fn debate(provider: &Arc<ScriptedProvider>, round_limit: u32) -> Result<DebateOutcome, DebateError> {
    let dynamic: Arc<dyn ModelProvider> = provider.clone();
    let advocate = spawn_agent(def("advocate"), dynamic.clone());
    let adversary = spawn_agent(def("adversary"), dynamic);
    let input = DebateInput { proposal: PROPOSAL.into(), context: None, round_limit };
    run_debate(&input, &advocate, &adversary, None, &CancellationToken::new()).await
}

fn critique(id: &str) -> String {
    format!(
        r#"{{"id": "{id}", "kind": "gap", "places": ["So C is true"], "point": "A and B do not give C", "support": "A and B hold while C fails"}}"#
    )
}

fn adversary(responses: &[String], critiques: &[String], satisfied: bool) -> String {
    format!(r#"{{"responses": [{}], "critiques": [{}], "satisfied": {satisfied}}}"#, responses.join(", "), critiques.join(", "))
}

fn dropping(id: &str) -> String {
    format!(r#"{{"critique": "{id}", "action": "drop", "reasons": "the reading is right"}}"#)
}

fn pressing(id: &str) -> String {
    format!(r#"{{"critique": "{id}", "action": "press", "reasons": "here is the case"}}"#)
}

fn accepting(id: &str) -> String {
    format!(r#"{{"critique": "{id}", "kind": "accept", "reasons": "the step needs a premise the proposal lacks"}}"#)
}

fn discussing(id: &str, ground: &str, quotes: &[&str]) -> String {
    let quotes: Vec<String> = quotes.iter().map(|q| format!("\"{q}\"")).collect();
    format!(
        r#"{{"critique": "{id}", "kind": "discuss", "ground": "{ground}", "reasons": "what fails is not shown", "quotes": [{}]}}"#,
        quotes.join(", ")
    )
}

fn advocate(answers: &[String]) -> String {
    format!(r#"{{"answers": [{}]}}"#, answers.join(", "))
}

fn status(outcome: &DebateOutcome, id: &str) -> CritiqueStatus {
    outcome.threads.iter().find(|t| t.critique.id == id).unwrap_or_else(|| panic!("{id} has no thread")).status
}

#[tokio::test]
async fn satisfied_in_round_one_stops_without_an_advocate_turn() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[], true));
    let out = debate(&p, 3).await.unwrap();
    assert_eq!(out.ending, Ending::Satisfied { round: 1 });
    assert_eq!(out.rounds_run, 1);
    assert!(out.threads.is_empty());
    assert_eq!(p.calls(), vec!["adversary"]);
}

#[tokio::test]
async fn accepted_critique_is_settled_in_one_round() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[accepting("K1")]));
    let out = debate(&p, 1).await.unwrap();
    assert_eq!(out.ending, Ending::LimitReached);
    assert_eq!(status(&out, "K1"), CritiqueStatus::Accepted);
    assert_eq!(out.threads[0].replies.len(), 1);
    assert_eq!(p.calls(), vec!["adversary", "advocate"]);
}

#[tokio::test]
async fn discussed_critique_is_open_at_the_limit() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[])]));
    let out = debate(&p, 1).await.unwrap();
    assert_eq!(out.ending, Ending::LimitReached);
    assert_eq!(status(&out, "K1"), CritiqueStatus::Open);
}

#[tokio::test]
async fn dropped_after_discussion_and_satisfied() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[])]));
    p.push("adversary", adversary(&[dropping("K1")], &[], true));
    let out = debate(&p, 3).await.unwrap();
    assert_eq!(out.ending, Ending::Satisfied { round: 2 });
    assert_eq!(status(&out, "K1"), CritiqueStatus::Dropped);
    let replies = &out.threads[0].replies;
    assert_eq!(replies.len(), 2);
    assert!(matches!(&replies[0], Reply::Advocate { round: 1, answer } if answer.kind == AnswerKind::Discuss));
    assert!(matches!(&replies[1], Reply::Adversary { round: 2, response } if response.action == ResponseAction::Drop));
    assert_eq!(p.calls(), vec!["adversary", "advocate", "adversary"]);
}

#[tokio::test]
async fn pressed_twice_then_accepted() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[])]));
    p.push("adversary", adversary(&[pressing("K1")], &[], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[])]));
    p.push("adversary", adversary(&[pressing("K1")], &[], false));
    p.push("advocate", advocate(&[accepting("K1")]));
    let out = debate(&p, 3).await.unwrap();
    assert_eq!(out.ending, Ending::LimitReached);
    assert_eq!(out.rounds_run, 3);
    assert_eq!(status(&out, "K1"), CritiqueStatus::Accepted);
    assert_eq!(out.threads[0].replies.len(), 5);
}

#[tokio::test]
async fn new_critique_in_a_later_round_is_answered() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[accepting("K1")]));
    p.push("adversary", adversary(&[], &[critique("K3")], false));
    p.push("advocate", advocate(&[accepting("K3")]));
    let out = debate(&p, 2).await.unwrap();
    let ids: Vec<&str> = out.threads.iter().map(|t| t.critique.id.as_str()).collect();
    assert_eq!(ids, vec!["K1", "K3"]);
    assert_eq!(status(&out, "K1"), CritiqueStatus::Accepted);
    assert_eq!(status(&out, "K3"), CritiqueStatus::Accepted);
    assert_eq!(out.threads[0].raised_in, 1);
    assert_eq!(out.threads[1].raised_in, 2);
}

#[tokio::test]
async fn cancelled_before_start_calls_no_model() {
    let p = Arc::new(ScriptedProvider::new());
    let dynamic: Arc<dyn ModelProvider> = p.clone();
    let advocate = spawn_agent(def("advocate"), dynamic.clone());
    let adversary = spawn_agent(def("adversary"), dynamic);
    let input = DebateInput { proposal: PROPOSAL.into(), context: None, round_limit: 2 };
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = run_debate(&input, &advocate, &adversary, None, &cancel).await.expect_err("the run should be cancelled");
    assert!(err.cancelled);
    assert!(p.calls().is_empty());
}
