# Cross-examine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `cross-examine`, a Rust binary whose one function cross-examines a proposal: an advocate reconstructs its argument as a tree, an adversary critiques it under explanatory-critique, and a report goes back to the caller over MCP (`serve`) or the command line (`run`). An `eval` command scores the tool against the Arguinas test split.

**Architecture:** One library crate with one binary. An orchestrator owns each run and talks to the advocate and the adversary, two actors that each run as a tokio task with an inbox and keep their own conversation. Code checks every tree and turn, analyzers compute the structural findings, and two thin front ends share one `cross_examine` function and one run record.

**Tech Stack:** Rust 2024 (rustc 1.95), tokio, rmcp 3.8 (official MCP SDK, stdio), reqwest 0.13, serde/serde_json, schemars 1, clap 4, toml 1, dirs 7, anyhow, thiserror, chrono, uuid, async-trait.

**Spec:** [SPEC.md](../../../SPEC.md)

## Global Constraints

- Pure Rust: Rust code and Rust crates only. No Python or Node runtime, no LangChain or LangGraph, no subprocess of any kind.
- Dependencies (`Cargo.toml`): `rmcp = { version = "3.8.0", features = ["server", "transport-io", "macros"] }`, `tokio = { version = "1.54", features = ["full"] }`, `tokio-util = "0.7"`, `reqwest = { version = "0.13.5", features = ["json"] }`, `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `schemars = "1"`, `clap = { version = "4.6", features = ["derive"] }`, `toml = "1"`, `dirs = "7"`, `anyhow = "1"`, `thiserror = "2"`, `chrono = "0.4"`, `uuid = { version = "1", features = ["v4"] }`, `async-trait = "0.1"`.
- Names: the calling agent is the **caller**, the reconstructing agent the **advocate**, the critiquing agent the **adversary**. Code, files, prompts and report headings use these names.
- In `serve` mode stdout carries only MCP messages. Every warning and log line goes to stderr.
- Round limit: set in configuration, never per call. Default 2, clamped to 1–3.
- DeepSeek defaults: base URL `https://api.deepseek.com`, model `deepseek-flash`, key from the environment variable `DEEPSEEK_API_KEY`, `response_format: {"type": "json_object"}`.
- A blank or syntactically invalid JSON reply is retried twice. A reply cut off at `max_tokens` (`finish_reason == "length"`) fails the turn at once.
- A turn that fails a check (tree, form, completeness, quotes) gets one retry with the failures. A second failure fails the run.
- Analyzer defaults: `stub_max_len = 2` claims, `uncovered_min_words = 4`.
- The advocate and the adversary have no tools, no web access and no file access.
- Testing: not test-driven, and compiling code counts as done. Tests exist only in Tasks 2, 4 and 7, which cover the tree checks, the analyzers and the ending of the debate, and they use a scripted model with no network. Every other task is verified with `cargo build` and `cargo clippy --all-targets`, which must report no warnings.

## Review Focus

These inputs break a run in ways the spec doesn't mention, and a person would expect sensible behaviour from each.

1. **Markdown or typographic characters in the proposal.** The advocate quotes `**bold**` text without its asterisks, or writes straight quotes where the text has curly ones. The retry message must name the quote that wasn't found and tell the advocate to copy it character for character, including markup. The test goes in Task 2.
2. **A long proposal whose JSON is cut off at `max_tokens`.** The run should fail at once, with a reason that says to raise `max_tokens` in the agent definition, and not spend retries. This is checked by hand in Task 5.
3. **An empty or whitespace-only proposal.** The run should fail before any model call or run record, with the reason "the proposal is empty". This is checked by hand in Task 9.
4. **A missing API key.** `run` and `serve` should fail at startup with a message that names the environment variable, before any proposal is read. This is checked by hand in Task 9.
5. **Cancellation mid-turn**, whether an MCP cancel or Ctrl-C. The run should stop, its status should become `cancelled`, the completed turns should stay in the record, and no report should be written. This is checked by hand in Task 10.

---

### Task 1: Scaffold, data model and configuration

**Files:**

- Create: `Cargo.toml`, `.gitignore` (`/target`), `src/main.rs`, `src/lib.rs`, `src/model.rs`, `src/config.rs`

**Interfaces:**

- Produces, in `src/model.rs`. Every type derives `Serialize, Deserialize, Clone, Debug`. Enums use the serde casing shown, and every `Vec`/`Option`/`bool` field has `#[serde(default)]`.

```rust
pub type ClaimId = String;      // "C1", "C2", …
pub type InferenceId = String;  // "I1", …
pub type CritiqueId = String;   // "K1", …

pub struct Origin { pub round: u32, pub critique: CritiqueId }
pub struct Claim { pub id: ClaimId, pub text: String, pub places: Vec<String>,
                   pub origin: Option<Origin>, pub backed_by_context: bool }
pub struct Inference { pub id: InferenceId, pub premises: Vec<ClaimId>, pub conclusion: ClaimId,
                       pub places: Vec<String>, pub origin: Option<Origin> }
pub struct Tree { pub claims: Vec<Claim>, pub inferences: Vec<Inference>, pub conclusions: Vec<ClaimId> }

#[serde(rename_all = "lowercase")] pub enum AnswerKind { Accept, Adapt, Reject }
#[serde(rename_all = "snake_case")] pub enum Ground { Stated, Follows, CounterFails }
pub struct Answer { pub critique: CritiqueId, pub kind: AnswerKind, pub ground: Option<Ground>,
                    pub reasons: String, pub quotes: Vec<String> }
pub struct AdvocateTurn { pub answers: Vec<Answer>, pub tree: Tree }

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target { Gap { inference: InferenceId },
                  Superfluous { inference: InferenceId, premise: ClaimId },
                  Reading { of: String } }                       // a claim or inference id
pub struct Critique { pub id: CritiqueId, #[serde(flatten)] pub target: Target,
                      pub point: String, pub support: String, pub quotes: Vec<String> }
#[serde(rename_all = "lowercase")] pub enum ResponseAction { Drop, Maintain }
pub struct Response { pub critique: CritiqueId, pub action: ResponseAction, pub reasons: String }
pub struct AdversaryTurn { pub responses: Vec<Response>, pub critiques: Vec<Critique>, pub satisfied: bool }

#[serde(tag = "role", rename_all = "lowercase")]
pub enum Turn { Advocate { round: u32, turn: AdvocateTurn }, Adversary { round: u32, turn: AdversaryTurn } }

pub struct Span { pub start: usize, pub end: usize }   // byte offsets into the original proposal
pub struct CheckedTree { pub tree: Tree,
                         pub claim_spans: BTreeMap<ClaimId, Vec<Span>>,
                         pub inference_spans: BTreeMap<InferenceId, Vec<Span>> }

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ending { Satisfied { round: u32 }, LimitReached }
#[serde(rename_all = "snake_case")]
pub enum CritiqueStatus { Accepted, Adapted, DroppedAfterRejection, Open, Unanswered }
pub struct LedgerRow { pub critique: Critique, pub raised_in: u32, pub status: CritiqueStatus }
pub struct Change { pub round: u32, pub critique: Critique, pub answer: Answer }
pub struct OpenCritique { pub critique: Critique, pub answer: Answer, pub response: Response }
pub struct DebateOutcome { pub turns: Vec<Turn>, pub final_tree: CheckedTree, pub ending: Ending,
                           pub rounds_run: u32, pub round_limit: u32, pub ledger: Vec<LedgerRow>,
                           pub changes: Vec<Change>, pub open: Vec<OpenCritique>, pub unanswered: Vec<Critique> }
```

- Produces, in `src/config.rs`, deriving `Serialize, Deserialize, Clone, Debug` with `#[serde(default)]`:

```rust
pub struct Config { pub rounds: u32, pub runs_dir: Option<PathBuf>,
                    pub providers: BTreeMap<String, ProviderConfig>, pub analyzers: AnalyzerConfig }
pub struct ProviderConfig { pub base_url: String, pub api_key_env: String }
pub struct AnalyzerConfig { pub stub_max_len: usize, pub uncovered_min_words: usize }
impl Config {
    pub fn load(path: Option<&Path>) -> anyhow::Result<Config>;
    pub fn round_limit(&self) -> u32;   // clamps to 1..=3; prints a warning to stderr when it clamps
    pub fn runs_dir(&self) -> PathBuf;  // runs_dir, else <dirs::data_dir()>/cross-examine/runs
}
pub fn config_dir() -> Option<PathBuf>; // <dirs::config_dir()>/cross-examine
```

- [ ] **Step 1: Write `Cargo.toml`** with package name `cross-examine`, edition 2024, the dependencies from Global Constraints, `[lib] name = "cross_examine"` and `[[bin]] name = "cross-examine"`.
- [ ] **Step 2: Write `src/model.rs`** with exactly the types above.
- [ ] **Step 3: Write `src/config.rs`.** `Default` gives `rounds = 2`, a `providers` map holding `"deepseek"` → `{ base_url: "https://api.deepseek.com", api_key_env: "DEEPSEEK_API_KEY" }`, and `AnalyzerConfig { stub_max_len: 2, uncovered_min_words: 4 }`. `load` tries the `path` argument first, then `./cross-examine.toml`, then `config_dir()/config.toml`, and otherwise uses `Config::default()`. A file's `providers` are merged over the default map, so a user file that adds a provider keeps `deepseek`.
- [ ] **Step 4: Write `src/lib.rs`** declaring the modules, and a `src/main.rs` with an empty `main` (Task 9 replaces it).
- [ ] **Step 5: Verify.** Run `cargo build && cargo clippy --all-targets`. Expected: builds, no warnings.
- [ ] **Step 6: Commit.** `git add -A && git commit -m "feat: scaffold crate, data model and configuration"`

---

### Task 2: Quote index and tree checks

**Files:**

- Create: `src/check/mod.rs`, `src/check/quotes.rs`, `src/check/tree.rs`
- Test: `tests/tree_check.rs`

**Interfaces:**

- Consumes: `Tree`, `CheckedTree`, `Span` from Task 1.
- Produces:

```rust
// src/check/quotes.rs
pub struct QuoteIndex { /* normalised proposal + map from normalised byte to original byte */ }
impl QuoteIndex {
    pub fn new(proposal: &str) -> Self;
    pub fn find_all(&self, quote: &str) -> Vec<Span>;   // every occurrence, in order; empty quote → empty Vec
    pub fn contains(&self, quote: &str) -> bool;
}
// src/check/tree.rs
pub fn check_tree(index: &QuoteIndex, tree: Tree) -> Result<CheckedTree, Vec<String>>;
```

Normalisation: every run of Unicode whitespace becomes one ASCII space, in both the proposal and the quote, and the quote is trimmed. Matching is otherwise exact: case, punctuation and markup all count. The spans returned point into the original proposal.

`check_tree` collects every failure as a message the advocate can act on and returns them all together:
- duplicate claim or inference ids;
- an inference with no premises;
- an inference premise, inference conclusion or `conclusions` entry that names no claim;
- an empty `conclusions` list ("no conclusion is marked");
- a loop, reported as the path of claim ids that forms it (a self-loop counts);
- a claim with no places and no `origin` ("claim C4 has no place and no origin");
- a place that does not occur in the proposal. The message quotes it and adds: "copy it from the proposal character for character, including markup and punctuation".

**Duplicate places.** When one claim lists the same normalised quote k times, the k places get the first k distinct occurrences. If there are fewer than k occurrences, that is a failure. Different claims may share spans.

- [ ] **Step 1: Write the tests in `tests/tree_check.rs`.** Use small helpers `claim(id, places)` and `inf(id, premises, conclusion)`.

```rust
#[test] fn quote_matches_across_whitespace() {
    let p = "Rain fell.\n\n  The  streets are wet.";
    let spans = QuoteIndex::new(p).find_all("The streets are wet");
    assert_eq!(spans.len(), 1);
    assert_eq!(&p[spans[0].start..spans[0].end], "The  streets are wet");
}
#[test] fn markup_must_be_copied() {            // Review Focus 1
    let idx = QuoteIndex::new("This is **essential** here.");
    assert!(idx.contains("**essential**"));
    assert!(!idx.contains("This is essential here"));
}
#[test] fn missing_place_fails_with_quote_in_message() {
    // claim C1 place "snow fell" in proposal "Rain fell."; conclusions [C1]
    // → Err(msgs) where some msg contains "snow fell" and "character for character"
}
#[test] fn duplicate_places_take_distinct_occurrences() {
    // proposal "Z holds. A is true, so Z holds."; C1 places ["Z holds","Z holds"] → Ok, two distinct spans;
    // with three "Z holds" places → Err
}
#[test] fn two_claim_loop_is_rejected()    { /* I1: C1→C2, I2: C2→C1 → Err containing "loop" */ }
#[test] fn self_loop_is_rejected()        { /* I1: [C1]→C1 → Err containing "loop" */ }
#[test] fn merging_trees_pass()           { /* X→Q, Y→Q, Q→Z, conclusions [Z] → Ok */ }
#[test] fn unknown_claim_reference_fails() { /* I1: [C9]→C1 → Err containing "C9" */ }
#[test] fn claim_without_place_or_origin_fails() { /* → Err containing "no place and no origin" */ }
#[test] fn added_claim_without_place_passes()    { /* places [], origin Some{round:2,critique:"K1"} → Ok */ }
#[test] fn no_conclusion_fails()          { /* conclusions [] → Err containing "no conclusion" */ }
```

- [ ] **Step 2: Implement `QuoteIndex` and `check_tree`.** Find loops with a three-colour DFS over the edges premise → conclusion, and rebuild the loop path from the DFS stack.
- [ ] **Step 3: Run the tests.** Run `cargo test --test tree_check`. Expected: all pass.
- [ ] **Step 4: Commit.** `git add -A && git commit -m "feat: quote index and tree checks"`

---

### Task 3: Turn checks, form and completeness

**Files:**

- Create: `src/check/turn.rs`

**Interfaces:**

- Consumes: `QuoteIndex`, `check_tree` (Task 2); `AdvocateTurn`, `AdversaryTurn`, `CheckedTree` (Task 1).
- Produces:

```rust
pub fn check_advocate_turn(index: &QuoteIndex, turn: AdvocateTurn, round: u32, expected: &BTreeSet<CritiqueId>)
    -> Result<(AdvocateTurn, CheckedTree), Vec<String>>;
pub fn check_adversary_turn(index: &QuoteIndex, turn: &AdversaryTurn, tree: &CheckedTree,
                            rejected: &BTreeSet<CritiqueId>, used_ids: &BTreeSet<CritiqueId>)
    -> Result<(), Vec<String>>;
```

`check_advocate_turn` collects every failure, together with those from `check_tree`:
- In round 1, `answers` must be empty and no claim may carry an `origin`.
- In later rounds, the `answers` must cover `expected` exactly, once each, naming any id that is missing or extra.
- Every answer needs non-empty `reasons`.
- A `reject` needs a `ground`. The `stated` ground needs at least one quote.
- Every quote must occur in the proposal (`index.contains`). A failure uses the same message as for a tree place.

`check_adversary_turn` collects every failure:
- **Critiques:**
  - Each critique needs a non-empty `id` that isn't in `used_ids` and isn't repeated within the turn.
  - Each needs non-empty `point` and `support`.
  - A `gap` must name an inference in `tree`.
  - A `superfluous` must name an inference in `tree`, one of that inference's premises, and an inference with at least two premises.
  - A `reading` must name a claim or an inference.
- **Responses:**
  - There must be a response for every id in `rejected`.
  - A `maintain` response needs non-empty `reasons`.
- **Verdict:**
  - `satisfied: true` with any critique or any `maintain` response contradicts the verdict.
  - `satisfied: false` with no critique and no `maintain` contradicts it as well.
- **Quotes:** every quote must occur in the proposal.

- [ ] **Step 1: Implement both functions** in `src/check/turn.rs` and re-export them from `src/check/mod.rs`.
- [ ] **Step 2: Verify.** Run `cargo build && cargo clippy --all-targets`. Expected: no warnings.
- [ ] **Step 3: Commit.** `git add -A && git commit -m "feat: form and completeness checks for advocate and adversary turns"`

---

### Task 4: Analyzers

**Files:**

- Create: `src/analyze/mod.rs`, `src/analyze/restatement.rs`, `src/analyze/uncovered.rs`, `src/analyze/stub.rs`
- Test: `tests/analyzers.rs`

**Interfaces:**

- Consumes: `CheckedTree`, `Span`, `ClaimId` (Task 1); `AnalyzerConfig` (Task 1).
- Produces:

```rust
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum Finding {
    Restatement { claim: ClaimId, quotes: Vec<String>, spans: Vec<Span> },
    Uncovered { text: String, span: Span },
    Stub { claims: Vec<ClaimId>, prune: bool },
}
pub trait Analyzer: Send + Sync {
    fn name(&self) -> &'static str;
    fn analyze(&self, proposal: &str, tree: &CheckedTree) -> Vec<Finding>;
}
pub struct Restatement;                                  // claims with more than one span
pub struct Uncovered { pub min_words: usize }
pub struct Stubs { pub max_len: usize }
pub fn default_analyzers(cfg: &AnalyzerConfig) -> Vec<Box<dyn Analyzer>>;
```

- **Uncovered.** Every claim span and inference span marks its bytes as covered. Each maximal uncovered stretch is trimmed, and its words are counted as whitespace-separated tokens that contain at least one alphanumeric character. A stretch with fewer than `min_words` words is ignored.
- **Stubs.** A claim is on the argument when it is a conclusion, or a premise of an inference whose conclusion is on the argument; this is computed to a fixpoint. The claims that are off the argument are grouped into connected components, where an inference joins all of its claims that are off the argument. Each component is reported as one `Stub`, with its claims in tree order and `prune = claims.len() <= max_len`. A premise removed by an accepted superfluous critique and used nowhere else is off the argument, so it appears here without special handling.

- [ ] **Step 1: Write the tests in `tests/analyzers.rs`.** They build `CheckedTree`s with `check_tree` from Task 2.

```rust
#[test] fn restatement_reports_every_place() {
    // proposal "Z holds. A is true. So Z holds." C3 places ["Z holds","Z holds"]
    // → exactly one Finding::Restatement { claim: "C3", quotes.len() == 2 }
}
#[test] fn uncovered_text_respects_minimum() {
    // proposal "Indeed, as we all know very well, rain falls. So streets get wet."
    // places: C1 "rain falls", C2 "streets get wet", I1 (C1→C2) "So"; min_words 4
    // → exactly one Uncovered whose text == "Indeed, as we all know very well,"
}
#[test] fn stubs_split_by_length() {
    // X→Z (conclusion Z); S1 alone; S2→S3, S3→S4 off the argument; max_len 2
    // → Stub{claims:["S1"], prune:true} and Stub{claims:["S2","S3","S4"], prune:false}, nothing else
}
#[test] fn merged_trees_have_no_stubs()   { /* X→Q, Y→Q, Q→Z → no Stub */ }
#[test] fn unused_conclusion_of_argument_is_stub() {
    // P→Z and P→S, conclusion Z → Stub{claims:["S"], prune:true}
}
```

- [ ] **Step 2: Implement the analyzers and `default_analyzers`.**
- [ ] **Step 3: Run the tests.** Run `cargo test --test analyzers`. Expected: all pass.
- [ ] **Step 4: Commit.** `git add -A && git commit -m "feat: restatement, uncovered-text and stub analyzers"`

---

### Task 5: Model provider

**Files:**

- Create: `src/provider/mod.rs`, `src/provider/openai.rs`, `src/provider/scripted.rs`

**Interfaces:**

- Consumes: `ProviderConfig` (Task 1).
- Produces:

```rust
pub struct ChatMessage { pub role: String, pub content: String }        // "system" | "user" | "assistant"
pub struct ChatRequest { pub model: String, pub messages: Vec<ChatMessage>, pub max_tokens: u32,
                         pub extra: serde_json::Map<String, serde_json::Value> }
pub struct Completion { pub content: String, pub finish_reason: Option<String> }
#[derive(thiserror::Error, Debug)]
pub enum ProviderError { #[error("request failed: {0}")] Http(String),
                         #[error("provider returned {status}: {body}")] Status { status: u16, body: String },
                         #[error("{0}")] Other(String) }
#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
    async fn complete(&self, req: &ChatRequest) -> Result<Completion, ProviderError>;
}
pub struct OpenAiCompatible { /* reqwest::Client, base_url, api_key */ }
impl OpenAiCompatible { pub fn from_config(cfg: &ProviderConfig) -> anyhow::Result<Self>; }
pub struct ScriptedProvider { /* Mutex<BTreeMap<model, VecDeque<String>>>, Mutex<Vec<String>> */ }
impl ScriptedProvider { pub fn new() -> Self; pub fn push(&self, model: &str, reply: impl Into<String>);
                        pub fn calls(&self) -> Vec<String>; /* models called, in order */ }
```

- **`OpenAiCompatible::from_config`** fails when the environment variable is unset or empty, with the message `"set the environment variable {api_key_env} to the provider's API key"` (Review Focus 4).
- **`complete`** POSTs to `{base_url}/chat/completions` with a bearer token and a 600-second timeout. The body is `{model, messages, max_tokens, response_format: {"type": "json_object"}}` with `extra` merged over it. The content and finish reason are read from `choices[0]`.
- **`ScriptedProvider`** returns the next queued reply for `req.model` with `finish_reason: Some("stop")`, and returns `ProviderError::Other("script exhausted for model …")` when the queue is empty.

- [ ] **Step 1: Implement the three files.**
- [ ] **Step 2: Verify.** Run `cargo build && cargo clippy --all-targets`. Expected: no warnings.
- [ ] **Step 3: Manual check (Review Focus 2), when `DEEPSEEK_API_KEY` is set.** Add a throwaway `#[ignore]` test or scratch binary that sends `max_tokens: 5` with a prompt asking for a long JSON object. Confirm `finish_reason == "length"`, then delete the scratch code.
- [ ] **Step 4: Commit.** `git add -A && git commit -m "feat: model provider trait, OpenAI-format client, scripted provider"`

---

### Task 6: Agent definitions and the bundled prompts

**Files:**

- Create: `src/agents.rs`, `agents/advocate.md`, `agents/adversary.md`, `agents/critique-standard.md`

**Interfaces:**

- Consumes: `config_dir()` (Task 1).
- Produces:

```rust
pub struct AgentDef { pub name: String, pub provider: String, pub model: String, pub max_tokens: u32,
                      pub request: serde_json::Map<String, serde_json::Value>, pub prompt: String,
                      pub source: String /* file path or "bundled" */ }
pub fn load_agent(name: &str) -> anyhow::Result<AgentDef>;   // "advocate" | "adversary"
pub fn parse_agent(name: &str, text: &str, source: String, standard: &str) -> anyhow::Result<AgentDef>;
```

**File format.** An agent file opens with TOML front matter between two `+++` lines, with the keys `provider`, `model`, `max_tokens` and an optional `[request]` table that is merged into the request body. One use of that table is `thinking = { type = "disabled" }`. The rest of the file is the system prompt, and `{{critique-standard}}` in it is replaced by the text of `critique-standard.md`.

**Lookup.** Each of the three files is looked up in this order, first match wins:
1. `./.cross-examine/agents/<file>`
2. `config_dir()/agents/<file>`
3. the bundled copy, compiled in with `include_str!("../agents/<file>")`. A `match` on the file name holds the bundled copies, and Task 11 adds `judge.md` to it.

- [ ] **Step 1: Write the three bundled files with exactly the text below.** The owner approves this wording as part of the plan review; it settles the spec's one open item.

`agents/critique-standard.md`:

```markdown
## The standard: explanatory-critique

The advocate and the adversary argue under one standard, and it binds both equally. Neither side is trying to win: together you are working out what the proposal argues and where that argument fails.

A critique takes one of three kinds:

1. gap: a critique of an inference names concretely what the inference misses and shows a case in which its premises hold and its conclusion fails. For X → Y, show that X can be true while Y is false, typically as X ∧ ¬Q → ¬Y, which exposes Q as a premise the inference assumes. This covers every form of argument: deductive, causal, analogical and definitional. A claim that X is necessary for Y is the inference ¬X → ¬Y and is critiqued the same way. An insufficient or inconclusive inference, a hidden premise and an invented connection are each shown by such a case.
2. superfluous: a critique of a premise shows that its inference does not need it, by deriving the inference's conclusion from the inference's other premises.
3. reading: a critique of a reading quotes the place in the proposal that was misread and gives the reading the text supports.

An objection without its target and its case, derivation or reading does not count. Raise none yourself, and say so when one is raised against you.

A rejection of a critique meets the standard in turn, on one of three grounds:

- stated: the proposal states the missing premise. Quote the place.
- follows: the missing premise follows from claims the proposal makes. Give the derivation.
- counter_fails: the critique's own premises can hold while its conclusion fails. Give that case. Against a superfluous critique, this is a case where the remaining premises hold and the conclusion fails, which shows the premise is needed.

A premise that is true but never stated is still missing. Its truth is no ground for rejection, because the argument rests on it and the proposal does not say so.

Copy every quote from the proposal character for character, including markup and punctuation.
```

`agents/advocate.md`:

```markdown
+++
provider = "deepseek"
model = "deepseek-flash"
max_tokens = 32768
+++
You are the advocate in a cross-examination of a proposal written by another agent, the caller. An adversary tests your reconstruction. You know only what the orchestrator sends you: the proposal, the caller's context and the adversary's turns.

{{critique-standard}}

## Your first turn: build the tree

1. Read every paragraph, sentence and phrase of the proposal and identify its substantive claims. Wording that makes no claim and draws no connection yields nothing.
2. Identify the inferences between the claims and record them in logical order, whatever order the text uses: "X because Y" is the inference from Y to X. An inference has one or more premises and exactly one conclusion.
3. Mark the claim or claims the proposal argues for as its conclusions.
4. Give every place where the proposal states each claim, as an exact quote. A claim stated twice has two places. For each inference, quote the words that state the connection, such as "because" or "so", when the text has them.
5. Stay faithful to the proposal. Record the connections it draws, including weak ones, and add nothing it does not say. Repair happens only in answer to the adversary.

## Later turns: answer the adversary

Answer every critique the adversary raised or maintained in its last turn with one of:

- accept: change the tree as the critique says. For a superfluous critique, remove the premise from the inference;
- adapt: change the tree differently, in a way that answers the critique;
- reject: give the ground (stated, follows or counter_fails) and the argument for it.

Give reasons for every answer, then give the full revised tree. A claim you add has no places: give it an origin naming the round and the critique that added it, and set backed_by_context to true when the caller's context supports it.

## Output

Reply with one JSON object and nothing else. In your first turn, answers is empty. The shape:

{"answers": [{"critique": "K1", "kind": "reject", "ground": "stated", "reasons": "…", "quotes": ["…"]}],
 "tree": {"claims": [{"id": "C1", "text": "…", "places": ["…"]},
                     {"id": "C4", "text": "…", "places": [], "origin": {"round": 2, "critique": "K2"}, "backed_by_context": false}],
          "inferences": [{"id": "I1", "premises": ["C1", "C2"], "conclusion": "C3", "places": ["because"]}],
          "conclusions": ["C3"]}}
```

`agents/adversary.md`:

```markdown
+++
provider = "deepseek"
model = "deepseek-flash"
max_tokens = 32768
+++
You are the adversary in a cross-examination of a proposal written by another agent, the caller. The advocate has turned the proposal into a tree of claims and inferences. You know only what the orchestrator sends you: the proposal, the caller's context and the advocate's turns.

{{critique-standard}}

## Each turn

1. Respond to every critique the advocate rejected: drop it when the advocate's answer meets the standard, or maintain it with your answer to the advocate's reasons. You may respond to an adaptation the same way.
2. Test every inference in the advocate's current tree. Where one fails, give a gap critique with the counter-case. Where an inference does not need one of its premises, give a superfluous critique with the derivation. Where you can show neither, raise nothing.
3. Test the advocate's reading against the proposal. Where it misread a place, or recorded a claim or connection the proposal does not make, give a reading critique that quotes the place.
4. Declare yourself satisfied when you maintain nothing and can show no new critique that meets the standard.

Give each new critique an id no earlier critique has used: K1, K2, and so on.

## Output

Reply with one JSON object and nothing else. The shape:

{"responses": [{"critique": "K1", "action": "maintain", "reasons": "…"}],
 "critiques": [{"id": "K3", "kind": "gap", "inference": "I2", "point": "what the inference misses", "support": "a case where its premises hold and its conclusion fails", "quotes": []},
               {"id": "K4", "kind": "superfluous", "inference": "I3", "premise": "C2", "point": "why I3 does not need C2", "support": "the derivation of I3's conclusion without C2", "quotes": []},
               {"id": "K5", "kind": "reading", "of": "C5", "point": "what was misread", "support": "the reading the text supports", "quotes": ["…"]}],
 "satisfied": false}
```

- [ ] **Step 2: Implement `load_agent` and `parse_agent`.** A missing key, or a missing or unterminated `+++` block, is an error that names the source file.
- [ ] **Step 3: Verify.** Run `cargo build && cargo clippy --all-targets`. Expected: no warnings.
- [ ] **Step 4: Commit.** `git add -A && git commit -m "feat: layered agent definitions and bundled prompts"`

---

### Task 7: Agent actors and the debate loop

**Files:**

- Create: `src/actor.rs`, `src/debate.rs`
- Test: `tests/debate_ending.rs`

**Interfaces:**

- Consumes: `ModelProvider`, `ChatRequest`, `ScriptedProvider` (Task 5); `AgentDef` (Task 6); `check_advocate_turn`, `check_adversary_turn`, `QuoteIndex` (Tasks 2–3); model types (Task 1).
- Produces:

```rust
// src/actor.rs
pub struct TurnReply { pub raw: String, pub value: serde_json::Value }
#[derive(thiserror::Error, Debug)] pub enum AgentError { /* Provider(ProviderError), Truncated, NoJson(attempts), Closed */ }
#[derive(Clone)] pub struct AgentHandle { /* mpsc::Sender<AgentMsg> */ }
pub fn spawn_agent(def: AgentDef, provider: Arc<dyn ModelProvider>) -> AgentHandle;
impl AgentHandle { pub async fn turn(&self, text: String) -> Result<TurnReply, AgentError>; }

// src/debate.rs
pub struct DebateInput { pub proposal: String, pub context: Option<String>, pub round_limit: u32 }
pub struct TurnEvent { pub index: u32, pub max: u32, pub turn: Turn, pub raw: String }
pub struct DebateError { pub reason: String, pub cancelled: bool, pub turns: Vec<Turn> }
pub async fn run_debate(input: &DebateInput, advocate: &AgentHandle, adversary: &AgentHandle,
                        events: Option<&mpsc::UnboundedSender<TurnEvent>>,
                        cancel: &CancellationToken) -> Result<DebateOutcome, DebateError>;
```

**The actor.** Each actor is one tokio task. It owns its history, which starts with the system prompt `def.prompt`, and handles one inbox message at a time:
1. Push the user message.
2. Call the provider with `extra = def.request`.
3. On `finish_reason == "length"`, return `Truncated` at once. The message says the reply was cut off at `max_tokens` and should be raised in the agent definition `def.source` (Review Focus 2).
4. On blank content or invalid JSON syntax, call again. After 3 attempts in total, return `NoJson`.
5. On success, push the reply as an assistant message and send it back.

**Turn order.** For each `r` in `1..=round_limit`, run the advocate's turn and then the adversary's turn. Stop after the first adversary turn with `satisfied: true`.

**What each agent receives.** Wording is free, but each message carries exactly this content:
- **Advocate, round 1:** the proposal under `## Proposal`, then the context under `## Context from the caller` (or "None."), then "Build the tree."
- **Adversary, round 1:** the same proposal and context, then the advocate's turn as pretty JSON under `## The advocate's turn, round 1`, then "Critique the tree."
- **Advocate, round r:** the adversary's last turn as JSON, then "Answer every critique raised or maintained above, then give the full revised tree."
- **Adversary, round r:** the advocate's turn as JSON, then "Respond to the advocate's rejections, then critique the revised tree."

**Checks.**
- For the advocate in round `r > 1`, `expected` is the ids of the adversary's last critiques plus the critiques it maintained in that turn.
- For the adversary, `rejected` is the ids the advocate rejected in this round's turn, and `used_ids` is every critique id raised so far.
- A reply that deserialises into the wrong shape, or fails `check_advocate_turn` or `check_adversary_turn`, gets one follow-up message: "Your reply failed these checks:", then the failures as a list, then "Reply again with the whole corrected JSON object." A second failure returns `DebateError`.

**After each accepted turn**, send a `TurnEvent` with `max = 2 * round_limit`.

**Cancellation.** Every `turn().await` races `cancel.cancelled()` in a `tokio::select!`. A cancellation returns `DebateError { cancelled: true, reason: "cancelled", turns }`.

**The ledger.** Walk the turns in order and track one status per critique id:
- **Raised** by the adversary in round r: `Unanswered`, with `raised_in = r`.
- **Advocate's answer:** `accept` → `Accepted`, `adapt` → `Adapted`, `reject` → pending rejection.
- **Adversary's `drop`:** a pending rejection becomes `DroppedAfterRejection`, and `Adapted` stays `Adapted`.
- **Adversary's `maintain`:** `Open`. A later answer from the advocate overwrites it.

Rows come out in the order the critiques were raised, each carrying its critique as raised.

**The outcome.**
- `changes`: every `accept` or `adapt` answer, paired with the critique it answers, found by id across all adversary turns.
- **Satisfied ending:** `Ending::Satisfied { round }`. `open` and `unanswered` are empty.
- **Limit ending:** `Ending::LimitReached`.
  - `open`: each `maintain` response in the adversary's last turn, paired with its critique and with the advocate's answer from the last advocate turn.
  - `unanswered`: the critiques of the adversary's last turn.
- `final_tree`: the `CheckedTree` from the last advocate turn.

- [ ] **Step 1: Write the tests in `tests/debate_ending.rs`.** Use one `ScriptedProvider` shared by both agents. The advocate's `AgentDef` uses model `"advocate"` and the adversary's uses model `"adversary"`. Prompts are arbitrary. The proposal is `"A is true. B is true. So C is true."`, and the advocate's tree quotes it.

```rust
#[tokio::test] async fn satisfied_in_round_one_stops_early() {
    // advocate: valid tree; adversary: {"satisfied": true}; round_limit 3
    // → ending Satisfied{round:1}, rounds_run 1, ledger empty, provider.calls() == ["advocate","adversary"]
}
#[tokio::test] async fn satisfied_in_round_two() {
    // adversary 1: gap K1 on I1; advocate 2: reject K1 (ground stated, quote "A is true");
    // adversary 2: drop K1, satisfied
    // → Satisfied{round:2}, open empty, unanswered empty, ledger K1 == DroppedAfterRejection
}
#[tokio::test] async fn limit_splits_open_and_unanswered() {
    // round_limit 2. adversary 1: K1, K2. advocate 2: reject K1 (stated, quote "A is true"),
    // accept K2 (adds C9 with origin {2,"K2"}). adversary 2: maintain K1 with reasons, new critique K3, satisfied false
    // → LimitReached; open ids == ["K1"]; unanswered ids == ["K3"]; changes ids == ["K2"]; rounds_run 2;
    //   ledger statuses: K1 Open, K2 Accepted, K3 Unanswered; K3 raised_in 2
}
#[tokio::test] async fn limit_one_leaves_every_critique_unanswered() {
    // round_limit 1. adversary 1: K1 → LimitReached, open empty, unanswered ids == ["K1"], ledger K1 Unanswered
}
```

- [ ] **Step 2: Implement `src/actor.rs` and `src/debate.rs`.**
- [ ] **Step 3: Run the tests.** Run `cargo test --test debate_ending`. Expected: all pass.
- [ ] **Step 4: Commit.** `git add -A && git commit -m "feat: advocate and adversary actors and the debate loop"`

---

### Task 8: Run record, report and the `cross_examine` function

**Files:**

- Create: `src/record.rs`, `src/report/mod.rs`, `src/report/markdown.rs`, `src/report/argdown.rs`, `src/run.rs`

**Interfaces:**

- Consumes: everything above.
- Produces:

```rust
// src/record.rs
pub struct RunRecord { pub id: String, pub dir: PathBuf }
impl RunRecord {
    pub fn create(runs_dir: &Path, proposal: &str, context: Option<&str>, config: &Config, agents: &[&AgentDef]) -> anyhow::Result<Self>;
    pub fn write_turn(&self, ev: &TurnEvent) -> anyhow::Result<()>;
    pub fn set_status(&self, status: &str) -> anyhow::Result<()>;   // "running" | "done" | "failed: <reason>" | "cancelled"
    pub fn write_report(&self, markdown: &str, json: &serde_json::Value, argdown: &str) -> anyhow::Result<()>;
}
// src/report/mod.rs
#[derive(Serialize)] pub struct Report { pub run_id: String, pub record_dir: PathBuf, pub ending: Ending,
    pub rounds_run: u32, pub round_limit: u32, pub ledger: Vec<LedgerRow>, pub final_tree: CheckedTree,
    pub changes: Vec<Change>, pub open: Vec<OpenCritique>, pub unanswered: Vec<Critique>,
    pub findings: Vec<Finding>, pub transcript: Vec<Turn> }
pub fn build_report(record: &RunRecord, outcome: DebateOutcome, findings: Vec<Finding>) -> Report;
// src/report/markdown.rs
pub fn render_markdown(report: &Report, proposal: &str) -> String;
pub fn render_transcript(turns: &[Turn]) -> String;
// src/report/argdown.rs
pub fn render_argdown(tree: &Tree) -> String;
// src/run.rs
pub struct Environment { pub config: Config, pub advocate: AgentDef, pub adversary: AgentDef,
                         pub providers: BTreeMap<String, Arc<dyn ModelProvider>> }
impl Environment { pub fn load(config_path: Option<&Path>) -> anyhow::Result<Self>; }
pub struct Progress { pub done: u32, pub max: u32, pub message: String }
pub struct RunOutput { pub report: Report, pub markdown: String, pub json: serde_json::Value, pub argdown: String }
pub enum RunFailure { Failed { reason: String, record_dir: Option<PathBuf>, transcript: String }, Cancelled { record_dir: PathBuf } }
pub async fn cross_examine(env: &Environment, proposal: String, context: Option<String>,
                           progress: Option<mpsc::UnboundedSender<Progress>>, cancel: CancellationToken)
    -> Result<RunOutput, RunFailure>;
```

- **Record layout.** The run id is `chrono::Local::now().format("%Y%m%d-%H%M%S")`, then `-`, then the first 6 hex characters of a v4 UUID. The folder `<runs_dir>/<id>/` holds:
  - `proposal.md`, and `context.md` when there is a context;
  - `config.toml`, the resolved `Config` written with `toml::to_string`;
  - `agents/<name>.md`, each with its resolved prompt and its source as the first line;
  - `turns/NN-<advocate|adversary>-r<round>.json` (the raw reply) and `turns/NN-<advocate|adversary>-r<round>.parsed.json`;
  - `status`;
  - `report.md`, `report.json` and `report.argdown`.
- **`Environment::load`.** Loads the config and both agents, and builds one `OpenAiCompatible` for each provider the agents name. An unknown provider name is an error.
- **`cross_examine`:**
  1. Reject a proposal that is empty after trimming with `Failed { reason: "the proposal is empty", record_dir: None, .. }` (Review Focus 3).
  2. Create the record and spawn both actors.
  3. Run the debate with the clamped `round_limit()`. A forwarding task writes each `TurnEvent` to the record and maps it to a `Progress` whose message is `"round {r}: {advocate|adversary} done"`.
  4. On success, run `default_analyzers` on the final tree, build the report, write all three renderings, and set the status to `done`.
  5. On a `DebateError`, set the status to `failed: …` or `cancelled`. A cancelled run writes no report (Review Focus 5).
- **Argdown mapping (`render_argdown`).**
  - Each inference, in tree order, becomes an argument titled `<I1>`. Directly under the title, with no blank line, come its premises as `(1) [C1]: text`, `(2) [C2]: text`, …, then a line of five hyphens, then its conclusion as `(n) [C3]: text`. A blank line separates arguments.
  - A claim used by no inference becomes a standalone `[C7]: text` line after the arguments.
  - Tags go at the end of the statement text: `#conclusion` for a marked conclusion, `#added` for a claim with an origin, `#backed-by-context` where that applies.
  - Characters in a claim's text are replaced so they can't be read as Argdown syntax: `[`→`(`, `]`→`)`, `<`→`‹`, `>`→`›`, `{`→`(`, `}`→`)`, `#`→`＃`, and `//`→`/ /`.
- **Markdown layout.** The headings, in order:
  - `# Cross-examination report`
  - `## Ending`
  - `## Critique ledger`, a table with the columns Critique, Kind, Target, Raised in and Status
  - `## Final tree`, holding the Argdown rendering in a ```` ```argdown ```` fenced block, then `### Places`, which lists each claim with its quoted places, or "added in round {r} for {K}", plus "(backed by context)" where that applies
  - `## Changes`
  - `## Open critiques`
  - `## Unanswered critiques`
  - `## Structural findings` with `### Restatements`, `### Uncovered text` and `### Stubs` (prune suggestions first)
  - `## Transcript`, with each turn headed `### Round {r}: advocate` or `### Round {r}: adversary`

  An empty section reads `None.`
- **Ending copy.** These sentences are exact:
  - Satisfied: "The adversary was satisfied in round {r} of at most {N}: it could show no critique of the final tree that meets explanatory-critique."
  - Limit: "The round limit of {N} was reached with {k} open and {m} unanswered critiques. Open critiques were argued by both sides. Unanswered critiques were raised in the adversary's last turn, and the advocate had no turn to examine them."
  - Both are followed by: "This describes how the debate ended. It is not a verdict on the proposal."

- [ ] **Step 1: Implement the five files.**
- [ ] **Step 2: Verify.** Run `cargo build && cargo clippy --all-targets`. Expected: no warnings.
- [ ] **Step 3: Commit.** `git add -A && git commit -m "feat: run record, report renderers and cross_examine"`

---

### Task 9: Command line, `run`

**Files:**

- Modify: `src/main.rs` (replace the Task 1 stub)

**Interfaces:**

- Consumes: `Environment`, `cross_examine`, `RunFailure` (Task 8); `serve` (Task 10, wired in that task).
- Produces:

```rust
#[derive(clap::Parser)] struct Cli { #[arg(long, global = true)] config: Option<PathBuf>, #[command(subcommand)] cmd: Cmd }
#[derive(clap::Subcommand)] enum Cmd {
    Serve,
    Run { proposal: PathBuf /* "-" reads stdin */, #[arg(long)] context: Option<PathBuf>,
          #[arg(long, value_enum, default_value_t = Format::Md)] format: Format, #[arg(long)] out: Option<PathBuf> },
}
#[derive(clap::ValueEnum, Clone)] enum Format { Md, Json, Argdown }
```

- **Startup.** `Environment::load` runs before the proposal is read, so a missing key fails first (Review Focus 4).
- **The run.** Ctrl-C (`tokio::signal::ctrl_c`) cancels the token.
- **Output.** On success the chosen rendering goes to stdout, or to `--out` when it is given. `argdown` prints the final tree only.
- **Exit codes:**
  - 0 when a report was produced, whatever the ending;
  - 1 on failure, with `cross-examination failed: {reason}` and the record directory on stderr;
  - 130 on cancellation.

- [ ] **Step 1: Implement `src/main.rs`.** `Serve` prints "not yet implemented" to stderr and exits 1 until Task 10.
- [ ] **Step 2: Verify the build.** Run `cargo build && cargo clippy --all-targets`. Expected: no warnings.
- [ ] **Step 3: Manual checks.**
  - `env -u DEEPSEEK_API_KEY cargo run -- run SPEC.md` should exit 1 with a message that names `DEEPSEEK_API_KEY`.
  - With the key set, `printf '  \n' | cargo run -- run -` should exit 1 with "the proposal is empty" and create no run folder.
  - With the key set, `cargo run -- run <a short proposal file>` should print a report and exit 0. Its run folder should hold every listed file, and `report.argdown` should parse in the Argdown sandbox or VS Code extension.
- [ ] **Step 4: Commit.** `git add -A && git commit -m "feat: run subcommand"`

---

### Task 10: MCP server, `serve`

**Files:**

- Create: `src/mcp.rs`
- Modify: `src/main.rs` (wire `Cmd::Serve`)

**Interfaces:**

- Consumes: `Environment`, `cross_examine`, `Progress`, `RunFailure`, `render_transcript` (Task 8).
- Produces:

```rust
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct CrossExamineArgs {
    /// The text to cross-examine, of any length. The advocate and the adversary see nothing else of your work, so state the argument in full.
    pub proposal: String,
    /// Optional: what you need the proposal for, and any evidence you already have. The advocate and the adversary argue from the proposal, this context and their own knowledge.
    pub context: Option<String>,
}
pub struct Server { env: Arc<Environment>, tool_router: rmcp::handler::server::tool::ToolRouter<Self> }
pub async fn serve(env: Environment) -> anyhow::Result<()>;   // Server::new(env).serve(rmcp::transport::stdio()).await?.waiting().await
```

- **The tool.** The `cross_examine` tool function lives in a `#[tool_router] impl Server` block. It takes `Parameters(args): Parameters<CrossExamineArgs>` and `ctx: RequestContext<RoleServer>`, and returns `Result<CallToolResult, rmcp::ErrorData>`. `#[tool_handler] impl ServerHandler for Server` implements `get_info` with tools enabled.
- **Progress.** When `ctx.meta.get_progress_token()` is `Some`, a task forwards each `Progress` as `ctx.peer.notify_progress(ProgressNotificationParam::new(token, done as f64).with_total(max as f64).with_message(message))`.
- **Cancellation.** `ctx.ct` is passed in as the cancellation token.
- **Results:**
  - On success: `CallToolResult::success(vec![ContentBlock::text(markdown)])`, with `structured_content = Some(json)`.
  - On `Failed`: `CallToolResult::error(vec![ContentBlock::text(format!("Cross-examination failed: {reason}. Run record: {dir}.\n\n{transcript}"))])`. When `record_dir` is `None`, the sentence "Run record: …" is left out.
  - On `Cancelled`: return `Err(ErrorData::internal_error("cancelled", None))`. The client discards it.
- **Tool description.** This is exact copy, part of the prompt review:

> Cross-examine a proposal for invented connections, unstated or unneeded premises, and text its argument does not use. An advocate reconstructs the proposal's argument as a tree of claims and inferences, and an adversary critiques it under explanatory-critique: a critique counts only if it shows a case where an inference's premises hold and its conclusion fails, derives a conclusion without one of its premises, or quotes a misreading. The advocate answers every critique, and the debate ends when the adversary is satisfied or a round limit is reached. The report holds a ledger of every critique, the final tree in Argdown, every change the debate forced, the critiques left open or unanswered, restatements, uncovered text and stubs to consider pruning, and the full transcript. It describes the proposal and does not judge it. The call can take several minutes.

- [ ] **Step 1: Implement `src/mcp.rs` and wire `Cmd::Serve`** to `Environment::load`, then `serve`.
- [ ] **Step 2: Verify the build.** Run `cargo build && cargo clippy --all-targets`. Expected: no warnings.
- [ ] **Step 3: Smoke test over stdio.** Pipe an `initialize` request, the `notifications/initialized` notification and a `tools/list` request into `cargo run -q -- serve`, one JSON-RPC message per line. Expected: stdout holds exactly two JSON responses, and the `tools/list` result names `cross_examine` with `proposal` required and `context` optional.
- [ ] **Step 4: Manual check (Review Focus 5).** In Claude Code with the server registered (`claude mcp add cross-examine -- <path>/target/debug/cross-examine serve`), call the tool on a short proposal. Confirm the report arrives. Then start a second call and interrupt it, and confirm the run folder's `status` reads `cancelled` and that it has no `report.md`.
- [ ] **Step 5: Commit.** `git add -A && git commit -m "feat: MCP server with the cross_examine tool"`

---

### Task 11: Arguinas evaluation, `eval`

**Files:**

- Create: `src/eval.rs`, `agents/judge.md`
- Modify: `src/agents.rs` (bundle `judge.md`), `src/main.rs` (add `Cmd::Eval`)

**Interfaces:**

- Consumes: `Environment`, `cross_examine`, `RunOutput`, `Report` (Task 8); `load_agent`, `AgentDef` (Task 6); `spawn_agent`, `AgentHandle` (Task 7); `OpenAiCompatible` (Task 5); model types (Task 1).
- Produces:

```rust
// src/eval.rs: only the dataset fields the evaluation reads
#[derive(Deserialize)] pub struct EvalRecord { pub title: String, pub background: String, pub argument: String,
    pub explicit_premises: Vec<String>, pub implicit_premises: Vec<String>,
    pub fallacy_info: Option<FallacyInfo>, pub sections: Sections }
#[derive(Deserialize)] pub struct FallacyInfo { pub rationale: String }
#[derive(Deserialize)] pub struct Sections { pub reconstruction: Reconstruction }
#[derive(Deserialize)] pub struct Reconstruction { pub conclusion: String }
#[derive(Serialize, Deserialize)] pub struct Judgement { pub conclusion: Option<ClaimId>,
    pub explicit: BTreeMap<String, Option<ClaimId>>, pub implicit_first: BTreeMap<String, Option<ClaimId>>,
    pub implicit_added: BTreeMap<String, Option<ClaimId>>, pub fallacy: Option<CritiqueId> }
#[derive(Serialize)] pub struct RecordScore { pub index: usize, pub run_id: Option<String>, pub error: Option<String>,
    pub ending: Option<Ending>, pub rounds_run: Option<u32>, pub conclusion_found: Option<bool>,
    pub explicit_in_first: Option<f64>, pub implicit_in_first: Option<f64>,
    pub implicit_recovered: Option<f64>, pub fallacy_identified: Option<bool> }
pub async fn run_eval(env: Arc<Environment>, dataset: &Path, limit: Option<usize>, concurrency: usize,
                      out: &Path, cancel: CancellationToken) -> anyhow::Result<Vec<RecordScore>>;
pub fn summarize(scores: &[RecordScore]) -> String;   // markdown
```

- **Inputs.** `--dataset` names a local copy of `260331-Arguinas-Test-v2.json` from <https://github.com/GyoukChu/Arguinas/tree/main/data>. The data is never committed. Each record runs `cross_examine` with the `argument` as the proposal and the context `"Debate topic: {title}"`, followed by a blank line and the `background` unless the background is `"None"`.
- **Concurrency.** Up to `concurrency` records run at once, default 4, limited by a `tokio::sync::Semaphore`. Ctrl-C cancels them all.
- **The judge.** A fresh judge actor is spawned for every record, from `load_agent("judge")` and the provider the judge names (taken from `env.providers`, or built from the config). One user message gives, each under its own heading with its id:
  - the reference conclusion; the explicit premises as `E1…`; the implicit premises as `H1…`; and the fallacy rationale when one exists;
  - our marked conclusions from `report.final_tree`;
  - the claims of the first advocate turn in `report.transcript`;
  - the claims of the final tree that have an origin;
  - the critiques in `report.ledger`, each with its id, kind and point.
- **Checking the judgement.** Every `E` and `H` key must be present, and every value must be null or an id from the list it was matched against. A judgement that fails gets one retry with the failures. A second failure records the error for that record and moves on.
- **Scores.** Each rate is the matched count over the reference count, and is `None` when the reference list is empty:
  - `explicit_in_first`: explicit premises matched in the first tree;
  - `implicit_in_first`: implicit premises matched in the first tree;
  - `implicit_recovered`: implicit premises matched among the added claims.

  `fallacy_identified` is `Some` only when the record has a fallacy label. A failed run records its error with every score `None`.
- **Output.** Each `RecordScore` is appended as one JSON line to `--out`, which defaults to `<runs_dir>/eval-<timestamp>.jsonl`, as soon as it is ready. At the end, `summarize` prints a markdown table to stdout:
  - one row per metric, with its mean and the number of records scored;
  - a note that the implicit-in-first-tree rate is better when lower;
  - counts of satisfied endings, limit endings and failed runs.
- **CLI.** Add `Eval { dataset: PathBuf, #[arg(long)] limit: Option<usize>, #[arg(long, default_value_t = 4)] concurrency: usize, #[arg(long)] out: Option<PathBuf> }` to `Cmd`.

- [ ] **Step 1: Write `agents/judge.md` with exactly this text,** and add it to the bundled `match` in `src/agents.rs`:

```markdown
+++
provider = "deepseek"
model = "deepseek-flash"
max_tokens = 8192
+++
You match statements for an evaluation. You receive reference statements from a published argument reconstruction and statements from a reconstruction under test, each with an id.

Two statements match when they state the same proposition: a reader would accept either one as saying what the other says. Wording, order and level of detail may differ. A statement that covers only part of the other, or adds a condition the other lacks, does not match. When several candidates match, choose the closest. When none matches, answer null.

Answer five questions:

- conclusion: which of the marked conclusions matches the reference conclusion;
- explicit: for each reference explicit premise (E1, E2, …), which claim of the first tree matches it;
- implicit_first: for each reference implicit premise (H1, H2, …), which claim of the first tree matches it;
- implicit_added: for each reference implicit premise, which added claim matches it;
- fallacy: when a fallacy rationale is given, which critique identifies the flaw it describes. A critique identifies the flaw when it targets the same step and names the same defect.

Reply with one JSON object and nothing else, in this shape:

{"conclusion": "C3", "explicit": {"E1": "C1", "E2": null}, "implicit_first": {"H1": null}, "implicit_added": {"H1": "C9"}, "fallacy": null}
```

- [ ] **Step 2: Implement `src/eval.rs` and wire `Cmd::Eval`.**
- [ ] **Step 3: Verify the build.** Run `cargo build && cargo clippy --all-targets`. Expected: no warnings.
- [ ] **Step 4: Manual check, with `DEEPSEEK_API_KEY` set.** Download the test split outside the repo and run `cargo run -- eval <path>/260331-Arguinas-Test-v2.json --limit 3`. Expected: three JSON lines in the output file, three run folders, and a summary table whose "scored" counts add up.
- [ ] **Step 5: Commit.** `git add -A && git commit -m "feat: eval command scoring against Arguinas"`
