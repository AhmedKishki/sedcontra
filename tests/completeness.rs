use std::sync::Arc;

use sedcontra::actor::spawn_agent;
use sedcontra::agents::AgentDef;
use sedcontra::config::ModelSettings;
use sedcontra::debate::{DebateError, DebateInput, run_debate};
use sedcontra::model::DebateOutcome;
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

fn critique_at(id: &str, place: &str) -> String {
    format!(
        r#"{{"id": "{id}", "kind": "gap", "places": ["{place}"], "point": "A and B do not give C", "support": "A and B hold while C fails"}}"#
    )
}

fn critique(id: &str) -> String {
    critique_at(id, "So C is true")
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

/// Pushes the same reply twice, so the retry fails as the first attempt did.
fn push_twice(p: &ScriptedProvider, model: &str, reply: String) {
    p.push(model, reply.clone());
    p.push(model, reply);
}

async fn failure(p: &Arc<ScriptedProvider>, round_limit: u32) -> String {
    debate(p, round_limit).await.expect_err("the run should fail").reason
}

#[tokio::test]
async fn advocate_must_answer_every_critique() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1"), critique("K2")], false));
    push_twice(&p, "advocate", advocate(&[accepting("K1")]));
    assert!(failure(&p, 1).await.contains("K2 has no answer"));
}

#[tokio::test]
async fn retry_after_a_failed_check_passes() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", r#"{"answers": [{"critique": "K1", "kind": "accept", "reasons": ""}]}"#);
    p.push("advocate", advocate(&[accepting("K1")]));
    assert!(debate(&p, 1).await.is_ok());
    assert_eq!(p.calls(), vec!["adversary", "advocate", "advocate"]);
}

#[tokio::test]
async fn adversary_must_respond_to_every_discussed_critique() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[])]));
    push_twice(&p, "adversary", adversary(&[], &[critique("K2")], false));
    assert!(failure(&p, 2).await.contains("discussed K1, but there is no response"));
}

#[tokio::test]
async fn round_one_takes_no_responses() {
    let p = Arc::new(ScriptedProvider::new());
    push_twice(&p, "adversary", adversary(&[dropping("K1")], &[critique("K1")], false));
    let reason = failure(&p, 1).await;
    assert!(reason.contains("names no critique the advocate discussed") && reason.contains("respond to none"), "{reason}");
}

#[tokio::test]
async fn satisfied_without_responding_fails() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[])]));
    push_twice(&p, "adversary", adversary(&[], &[], true));
    assert!(failure(&p, 2).await.contains("discussed K1, but there is no response"));
}

#[tokio::test]
async fn satisfied_turn_cannot_press() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[])]));
    push_twice(&p, "adversary", adversary(&[pressing("K1")], &[], true));
    assert!(failure(&p, 2).await.contains("satisfied is true"));
}

#[tokio::test]
async fn critique_id_cannot_be_reused() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    p.push("advocate", advocate(&[accepting("K1")]));
    push_twice(&p, "adversary", adversary(&[], &[critique("K1")], false));
    assert!(failure(&p, 2).await.contains("already used"));
}

#[tokio::test]
async fn answer_to_a_dropped_critique_is_rejected() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1"), critique("K2")], false));
    p.push("advocate", advocate(&[discussing("K1", "unclear", &[]), discussing("K2", "unclear", &[])]));
    p.push("adversary", adversary(&[dropping("K1"), pressing("K2")], &[], false));
    push_twice(&p, "advocate", advocate(&[accepting("K1"), accepting("K2")]));
    assert!(failure(&p, 2).await.contains("K1 is not a critique you were asked to answer"));
}

#[tokio::test]
async fn misreads_needs_a_quote() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    push_twice(&p, "advocate", advocate(&[discussing("K1", "misreads", &[])]));
    assert!(failure(&p, 1).await.contains("misreads"));
}

#[tokio::test]
async fn place_must_occur_in_the_proposal() {
    let p = Arc::new(ScriptedProvider::new());
    push_twice(&p, "adversary", adversary(&[], &[critique_at("K1", "snow fell")], false));
    let reason = failure(&p, 1).await;
    assert!(reason.contains("snow fell") && reason.contains("character for character"), "{reason}");
}

#[tokio::test]
async fn quotes_in_answers_are_checked() {
    let p = Arc::new(ScriptedProvider::new());
    p.push("adversary", adversary(&[], &[critique("K1")], false));
    push_twice(&p, "advocate", advocate(&[discussing("K1", "misreads", &["not in text"])]));
    assert!(failure(&p, 1).await.contains("not in text"));
}
