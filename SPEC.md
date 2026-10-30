# Adversarial cross-examination: spec

Status: requirements approved by the owner, 2026-10-30, and revised the same day with the names caller, advocate and adversary, the critique ledger, the superfluous-premise critique, the Argdown rendering and the Arguinas evaluation. The prompt wording in the implementation plan is approved. Nothing is built yet.

## Why this exists

An agent that writes a proposal draws every connection in it from its own model, so it has nothing outside itself to check those connections against. A connection it invented therefore reads exactly like one that holds. Text without any connection passes just as easily: a sentence that links to nothing, a conclusion announced before its argument, or a "not only X but also Y" where X and Y have no relation reads as fluently as an argument, and these are the marks of AI writing.

The application counters both with an independent cross-examination. One agent, the advocate, reconstructs the proposal's argument as explicit inferences, and a second agent, the adversary, isolated from the first, attacks each inference under a shared standard of critique. Their debate shows the agent that wrote the proposal where it invents a connection, rests on a premise it never states, carries a premise its argument does not need, or carries text the argument does not use.

## What the application is

The application is a box that holds two boxes. The outer box, the orchestrator, takes a proposal from a calling agent, the caller, and runs the cross-examination. Inside it, the advocate reconstructs the proposal's argument as a tree of inferences, and the adversary critiques that tree. They debate under one shared standard, explanatory-critique, until the adversary is satisfied or a round limit is reached, and the orchestrator returns a report to the caller.

The caller is autonomous. What it does with the report lies outside this project, whose product is the analysis, so the report describes the proposal and never rules on it.

The application has one function, `cross_examine`, which any agent can reach in two ways: as a tool on an MCP server, or as a command-line process that the caller's harness runs in the background.

## Terms

The structure the advocate builds is a directed graph without cycles. This spec calls it a tree, as the owner does.

| Term | Meaning |
| --- | --- |
| Proposal | The text the caller submits, of any length and subject. |
| Claim | A substantive proposition the advocate finds in the proposal; a node of the tree. |
| Inference | A connection `P, Q → Z` from one or more premise claims to one conclusion claim; an edge of the tree. |
| Place | An exact quote from the proposal that states a claim or an inference. |
| Root | A claim that no inference concludes: a premise the proposal starts from. |
| Leaf | A claim that no inference uses as a premise. |
| Conclusion | A claim the advocate marks as what the proposal argues for. |
| Stub | A group of claims, connected by inferences among themselves, from which no inference path reaches a conclusion. Its length is its number of claims. |
| Restatement | A claim the proposal states in more than one place. |
| Uncovered text | A stretch of the proposal that no place quotes. |
| Counter-proposal | A critique by the adversary, stated as a logical proposition set against one of the advocate's inferences, premises or readings. |
| Round | One advocate turn followed by one adversary turn. |

Two trees may start at different roots X and Y and meet at a shared claim Q. No path may loop back on itself.

## Decided behaviour

### The boxes

- The orchestrator owns the run. It passes messages between the advocate and the adversary, checks every tree the advocate produces, computes the findings, writes the run record and builds the report.
- The advocate and the adversary are actors. Each owns its own context, takes work only as messages from the orchestrator, and never reads or changes the other's state. In Rust each is its own task with an inbox.
- Neither has tools, web access or file access. They argue from the proposal, the caller's context, the debate so far and their own knowledge, so the caller gathers any evidence first and passes it in `context`.
- Within a run the turns are sequential, because each turn answers the one before. Separate runs share nothing, so any number can run at once without coordination.

### Building the tree

The advocate's first turn turns the proposal into a tree.

1. The advocate reads every paragraph, sentence and phrase and identifies the substantive claims. Wording that carries no claim and no connection yields nothing.
2. It identifies the inferences between claims and records them in logical order, whatever order the text uses: "X because Y" becomes `Y → X`. The tree is therefore in logical order by construction, and nothing judges the order of the text itself.
3. It marks the conclusion or conclusions the proposal argues for.
4. It records every place that states each claim. For each inference it records the place that states the connection, such as the "because", when the text states one.
5. It stays faithful to the caller. The first tree records the connections the proposal draws, including weak ones, and adds nothing the proposal did not say. Repair happens only in the debate, where every change traces back to a counter-proposal.

The orchestrator then checks the tree in code:

- every inference refers to existing claims;
- no path loops back on itself;
- every place occurs in the proposal word for word, with whitespace normalised, and the orchestrator records where;
- every claim taken from the proposal has at least one place;
- at least one conclusion is marked.

When a check fails, the advocate gets the failures and one retry, and a second failure fails the run. The same checks apply to every revised tree the advocate submits during the debate. A claim the advocate adds during the debate has no place; it records instead the round and the counter-proposal that added it.

### Explanatory-critique

The advocate and the adversary argue under one standard, and it binds both sides equally. A critique takes one of three kinds.

- **Gap.** A critique of an inference names concretely what the inference misses and shows a case in which its premises hold and its conclusion fails. For `X → Y` it shows that X can be true while Y is false, typically as `X ∧ ¬Q → ¬Y`, which exposes Q as a premise the inference assumes. The standard covers every form of argument: deductive, causal, analogical and definitional. A claim that X is necessary for Y is the inference `¬X → ¬Y` and is critiqued the same way. An insufficient or inconclusive inference, a hidden premise and an invented connection are each shown by such a case.
- **Superfluous premise.** A critique of a premise shows that the inference does not need it, by deriving the inference's conclusion from its other premises. A counter-case cannot expose such a premise, because the inference holds with or without it. Accepting the critique removes the premise from the inference; if no other inference uses it, the stub finding then reports it for pruning.
- **Reading.** A critique of the advocate's reading quotes the place the advocate misread and gives the reading the text supports.

An objection without its named target and its case, derivation or reading does not count. Code checks only the form: each critique names its kind, its target, its point and its support, and each rejection names its ground. A turn that fails the form check gets one retry, and a second failure fails the run. The other side judges the substance under the same rules, so no third agent referees.

### The debate

The turn order is advocate 1, adversary 1, advocate 2, adversary 2, … so that the adversary closes every round.

- **Round 1.** The advocate builds the tree. The adversary critiques it with counter-proposals, or declares itself satisfied.
- **Later rounds.** The advocate answers every counter-proposal from the adversary's last turn and submits the revised tree. The adversary then responds.
- **The advocate's answers.** The advocate accepts a counter-proposal and changes the tree as it says, adapts it by changing the tree differently in a way that answers the critique, or rejects it. Every answer gives its reasons. A rejection meets explanatory-critique in turn, on one of three grounds: the proposal states the missing premise at a quoted place, the premise follows from claims the proposal makes, or the counter-proposal's own premises can hold while its conclusion fails. The last ground also answers a superfluous-premise critique: a case where the remaining premises hold and the conclusion fails shows the premise is needed.
- **True is not stated.** The advocate may not reject a critique because the missing premise is true. The argument still rests on a premise the proposal never states, and exposing such premises is the report's purpose. The advocate accepts the critique and adds the premise, marking it as backed by the caller's context when the context supports it.
- **Quotes are checked.** Every quote in a rejection or in a reading critique must occur in the proposal word for word, under the same check as the places in the tree.
- **The adversary's response.** The adversary drops each rejected critique or maintains it with an answer to the advocate's reasons, checks each adaptation, and critiques the revised tree afresh. It declares itself satisfied when it can show no critique that meets the standard.
- **Completeness.** The orchestrator checks that the advocate answered every counter-proposal and that the adversary answered every rejection. A turn that skips one gets one retry, and a second skip fails the run.
- **Ending.** The debate ends at the first adversary turn that declares satisfaction, or when the round limit is reached. When the adversary is not satisfied in its last turn, its critiques fall into two groups. A critique it maintained against the advocate's rejection has been argued by both sides and is an open critique. A critique new in that turn never reached the advocate, so the report labels it unanswered.
- **Round limit.** Set in configuration and never per call: default 2, range 1–10, and a value outside the range is clamped with a warning so a mistyped setting cannot run without end. The evaluation sets its own limit of 10 through `eval --rounds`.

Every turn sees the proposal, the caller's context and every earlier turn of the debate.

### What the report contains

| Field | Content |
| --- | --- |
| Run | the run id and the location of its record |
| Ending | the adversary satisfied in round r of N, or the round limit reached with open or unanswered critiques. It describes how the debate ended and is never a verdict. |
| Critique ledger | one row per critique: its kind, its target, the round that raised it and its final status, which is accepted, adapted, dropped after rejection, open or unanswered |
| Final tree | claims, inferences and conclusions, each marked with its places in the proposal or with the round and counter-proposal that added it |
| Changes | every accepted or adapted counter-proposal and what it changed. An added claim is a premise the proposal needed and did not state; a removed premise is one it did not need. |
| Open critiques | when the limit was reached: the critiques the adversary maintained against the advocate's rejection, each with both sides' arguments |
| Unanswered critiques | when the limit was reached: the critiques new in the adversary's last turn, listed apart because the advocate had no turn to examine them |
| Structural findings | see below |
| Transcript | every turn, headed by round and role |

The report comes in three renderings of the same data: markdown for an agent or a person to read, JSON for a program, and Argdown for the final tree. Argdown is an established syntax for premise–conclusion structures, so the tree also opens as an argument map in Argdown's tools, and the markdown report shows its final tree in the same syntax.

### Structural findings

The findings take over what the owner first planned as a separate prune function. Each finding is computed in code by an analyzer that reads the final tree and the proposal, without a model call. They run on the final tree, so any misreading the adversary caught is already corrected.

- **Restatement.** The report quotes every place of the claim and suggests keeping one. A conclusion announced before its argument shows up here, because the advocate records the announcement and the final statement as one claim.
- **Uncovered text.** The report quotes each stretch of the proposal that no place covers as a candidate for cutting. Stretches shorter than a configured minimum, 4 words by default, are ignored.
- **Stub.** The report lists each stub with its claims and places. A stub no longer than a configured threshold, 2 claims by default, is marked as a prune suggestion; a longer one is reported as a separate line of argument.

### Front ends

Both front ends are thin layers over the same core and write the same run record. One binary serves both: `serve` starts the MCP server and `run` runs one cross-examination.

#### MCP server

- Stdio transport, built on `rmcp`, the official Rust MCP SDK, which implements the 2026-07-28 MCP spec.
- One tool, `cross_examine`, with two fields:
  - `proposal` (string, required): the text to cross-examine;
  - `context` (string, optional): what the caller needs the proposal for and any evidence it already has.
- The call waits until the report is ready, as the owner decided. When the client supplies a progress token, the server sends a progress notification after every turn, so the client sees the run advancing.
- The result carries the markdown report as text content and the JSON report as structured content.
- A failed run returns an error result with the reason and every completed turn.
- A cancelled request stops the run. MCP sends no result for a cancelled request, so the completed turns remain only in the run record.
- A client whose tool-call time limit is shorter than a debate cuts the call off, and such a client should use the command line. `rmcp` also implements MCP's tasks extension, which lets a call return at once and deliver its result later; it is a later option and stays out of the first build.

#### Command line

- `run <proposal file, or - for stdin> [--context <file>] [--format md|json|argdown] [--out <file>]`. The `argdown` format prints the final tree only.
- It prints the report to stdout, or writes it to `--out`, and always writes every rendering into the run record.
- It exits with 0 whenever a report was produced, whatever the ending, and with a non-zero code on failure, printing the reason to stderr.
- This is the background form: the caller's harness starts the process in the background and reads the report when it exits. Harnesses such as Claude Code notify the agent when a background process exits.

### Run record

Each run gets one folder under a configured runs directory. It holds the proposal and context, a snapshot of the resolved configuration and agent definitions, every turn as it completes (the raw model output and the parsed data), the status (running, done, failed or cancelled) and the report in every rendering. Because every turn is written when it completes, a later checkpoint-and-resume feature can restart a run from its last turn.

### The core and the swappable parts

The owner wants every decision in this spec to stay changeable without rewriting the core. The core is built first:

- the data model: proposal, claims, inferences, places, tree, counter-proposals, answers, turns and report;
- the debate loop, with its turn order, completeness checks, early stop and round limit;
- the tree checks.

Every other part sits behind one interface, so it can be replaced, and new parts of the same kind can be added alongside:

| Part | First implementation |
| --- | --- |
| Model provider | a client for OpenAI-format chat APIs, configured for DeepSeek |
| Agent prompts | one Markdown file each for the advocate and the adversary, holding only the prompt |
| Critique standard | explanatory-critique, in its own file that both prompts include |
| Analyzers | restatement, uncovered text, stub |
| Front ends | MCP server, command line |
| Report renderers | markdown, JSON, Argdown |

Agent definitions are looked up in layers, as in [pi-interactive-subagents](https://github.com/amosblomqvist/pi-interactive-subagents): a project's file overrides the user's, and the user's overrides the default compiled into the binary. The agents are independent of the provider: a prompt names no provider and no model. The configuration assigns each agent its model in its own `[models.<agent>]` table. The advocate and the adversary both default to `deepseek-flash`, and either can run on a different model or provider without touching the other. The built-in defaults live in a bundled configuration file rather than in code, and a user's file overrides them table by table; it also holds the provider settings, the round limit, the stub threshold, the uncovered-text minimum and the runs directory.

### Model access

- The first provider is DeepSeek, through its OpenAI-format API at `https://api.deepseek.com`. The default model is `deepseek-flash`, the cheap one, since the owner prefers a cheap model; `deepseek-v4-pro` is the alternative. The API key comes from a key file that `cross-examine key set <provider>` writes with mode 600, `~/.config/cross-examine/keys/<provider>` by default, and the tool refuses a key file other users can read. The environment variable named in the configuration, `DEEPSEEK_API_KEY` by default, is a fallback for CI, read only when no key file exists.
- The key must never reach a model. It travels only in the HTTP `Authorization` header and never enters a prompt, and every provider error is redacted and shortened before it can reach a report, an MCP result, stderr or the run record.
- The advocate and the adversary return structured data for the orchestrator to check, so every turn requests DeepSeek's JSON output mode, `response_format: {"type": "json_object"}`. The mode requires the prompt to mention JSON and show an example of the structure. It can also return blank content when the model's output was not valid JSON, so the provider retries a blank or unparsable reply twice before the turn fails.
- A reply that reaches `max_tokens` is not a failure. The cut-off reply stays in the agent's history, the agent is told to stop deliberating and give its decision now, and only a second cut-off in the same turn fails the run.
- Both DeepSeek models run in thinking mode. By default the advocate reasons at effort `low` and the adversary at `high`. Each agent's depth is set apart from the other's, in its own `[models.<agent>.request]` table: `reasoning_effort` (`low`, `high` or `max`), or `thinking = { type = "disabled" }` to turn reasoning off. These are the provider's own parameters, so they live in the configuration and not in code or prompts.

## Prior art

- The [adversarial agents example in Microsoft's MCP for Beginners](https://github.com/microsoft/mcp-for-beginners/blob/main/05-AdvancedTopics/mcp-adversarial-agents/README.md) debates a proposition between a FOR and an AGAINST agent and lets a third agent, a judge, give a verdict with a confidence score. This design keeps its per-agent histories and drops the judge, the assigned sides and the shared tools, because the report must never read as a verdict and the two sides work out one question together.
- [Elenchus](https://github.com/jhlee0409/elenchus-mcp) tracks every issue in a Verifier–Critic debate through explicit states, which the critique ledger adopts.
- GAAR ([Ryu et al., 2026](https://arxiv.org/abs/2603.17432)) reconstructs arguments with their implicit premises and reduces them to minimal premise sets, which the superfluous-premise critique adopts.

## Evaluation

An `eval` command measures the argument half of the tool against the test split of [Arguinas](https://github.com/GyoukChu/Arguinas): 241 debate arguments, each with a reconstruction that GAAR made on Claude Sonnet 4.5. Each record runs one ordinary cross-examination, with the argument as the proposal and the debate topic and background as context. A judge agent, defined like the advocate and the adversary, then matches the reference reconstruction against two trees of the run, the advocate's first tree and the final tree: for the conclusion, each explicit premise and each implicit premise, which claim of the tree states it.

| Measure | Our output | Reference |
| --- | --- | --- |
| Conclusion, explicit premises and implicit premises in the first tree | the claims of the advocate's first tree | `conclusion`, `explicit_premises`, `implicit_premises` |
| The same three in the final tree | the claims of the final tree | the same |
| Reference conclusion marked as a conclusion | the conclusions the final tree marks | `conclusion` |
| Stopped one step short, when the final tree lacks the reference conclusion | a conclusion the final tree marks states a reference premise | `explicit_premises`, `implicit_premises` |
| Fallacy identified, on the 49 records with a fallacy label | the critiques in the ledger | `fallacy_info.rationale` |

The first tree states what the text says, so a low share of implicit premises there is intended; the rise from the first tree to the final tree is what the debate added. The judge's matches are kept in each record's score, so a match can be checked against the trees in the run folder.

- The reference reconstructions are model output, so the scores measure agreement with GAAR, not correctness. GAAR also forces every argument into deductive validity and adds bridging premises that a causal or analogical argument does not need, so a recovery rate below 1 is expected.
- The data supports no measure of superfluous premises. In 131 of the 241 records the streamlined premise list drops the premise labels, and the 110 labelled records contain 7 unneeded premises in all.
- The data has no labels for uncovered text, restatements or stubs, so the pruning half of the tool goes unmeasured.
- Neither the repository nor the Hugging Face card states a license, so the data is downloaded for local use and never committed.

## Constraints

- Pure Rust. Every part is Rust code or a Rust crate, so the application uses no Python or Node runtime, no LangChain or LangGraph, and no subprocess: nothing shells out to `claude`, `codex` or any other program.
- The async runtime is tokio, which `rmcp` is built on.
- The MCP server runs over stdio.
- Testing. The build is not test-driven, and code that compiles counts as done. Tests cover only the areas where compiling code can still produce a wrong report without any visible error, and they use a scripted model with no network. Those areas are:
  - the tree checks, especially loop detection and the word-for-word quote check;
  - the analyzers, on hand-built trees;
  - the ending of the debate: the early stop at the first satisfied adversary turn and the split into open and unanswered critiques.
  - the key handling: owner-only storage, the refusal of a key file others can read, and redaction.

## Non-goals

- Judging the proposal. The report describes and the caller decides.
- Anything the caller does with the report.
- Judging the order of the proposal's text. The tree is rebuilt in logical order, and the restatement finding catches a conclusion stated before its argument.
- A separate prune function. Prune suggestions are part of the report.
- Retrieval or evidence gathering by the advocate or the adversary.
- Checkpoint and resume in the first build. The run record prepares for them.

## Anti-goals

These count as failure even if everything else works:

- The advocate's first tree repairs or extends the proposal, so the findings describe the advocate's argument instead of the proposal's.
- An objection that does not meet explanatory-critique changes the tree or stands at the end.
- An unanswered critique reads as one that survived the advocate's answer.
- A counter-proposal goes unanswered, or a rejection stands without reasons.
- The report reads as a verdict on the proposal.

## Left to the builder

- Internal structure and naming, the binary's name, the configuration format and the location of the runs directory.
- How each box holds its conversation: whether an actor keeps its own message history across turns or receives the whole debate each turn, provided it receives nothing except what the orchestrator sends.
- The JSON shapes of the advocate's and the adversary's turns.
