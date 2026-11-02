# Sed Contra: spec

## Why this exists

An agent that writes a proposal draws every connection in it from its own model, so it has nothing outside itself to check those connections against. A connection it invented therefore reads exactly like one that holds, and so does a premise it never states, a conclusion it assumes on the way to proving it, two sentences that cannot both hold, or a stretch of text its argument never uses.

The application counters this with an independent cross-examination. One agent, the adversary, raises every critique of the proposal's argument it can substantiate, and a second agent, the advocate, isolated from the first, defends the proposal from its own words. Both argue under one discipline, so a critique that shows a failure is accepted and a critique that misreads the proposal or shows nothing is dropped. What settles shows the agent that wrote the proposal where its argument rests on a premise it never states, presupposes what it concludes, contradicts itself, shifts the sense of a word or carries text it does not need.

## What the application is

The application is a box that holds two boxes. The outer box, the orchestrator, takes a proposal from a calling agent, the caller, and runs the cross-examination. Inside it, the adversary critiques the proposal and the advocate answers each critique. They debate under one discipline until the adversary is satisfied or a round limit is reached, and the orchestrator returns a report to the caller.

The caller is autonomous. What it does with the report lies outside this project, whose product is the analysis, so the report describes the proposal and never rules on it.

The application has one function, `videtur_quod`, which any agent can reach in two ways: as a tool on an MCP server, or as a command-line process that the caller's harness runs in the background.

## Terms

| Term | Meaning |
| --- | --- |
| Proposal | The text the caller submits, of any length and subject. It is the reference for everything either side says. |
| Step | An inference the proposal draws from one or more premises to a conclusion, in the proposal's own words. |
| Place | An exact quote from the proposal. A critique names what it targets by its places. |
| Critique | The adversary's claim that a step or a stretch of the proposal fails, with the support that shows it. |
| Answer | The advocate's reply to a critique: it accepts the critique or discusses it. |
| Response | The adversary's reply to an answer that discusses: it drops the critique or presses it. |
| Settled critique | A critique the advocate accepted or the adversary dropped. |
| Open critique | A critique still under discussion when the debate ends. |
| Round | One adversary turn followed by one advocate turn. |

## Decided behaviour

### The boxes

- The orchestrator owns the run. It passes messages between the adversary and the advocate, checks every turn, keeps the ledger, writes the run record and builds the report.
- The advocate and the adversary are actors. Each owns its own context, takes work only as messages from the orchestrator, and never reads or changes the other's state. In Rust each is its own task with an inbox.
- Neither has tools, web access or file access. They argue from the proposal, the caller's context, the debate so far and their own knowledge, so the caller gathers any evidence first and passes it in `context`.
- Within a run the turns are sequential, because each turn answers the one before. Separate runs share nothing, so any number can run at once without coordination.

### The discipline

The adversary and the advocate argue under one discipline, and it binds both equally. Neither tries to win: together they work out where the proposal's argument fails and where it holds. The proposal is the reference for everything either side says, so every critique and every answer quotes it.

A critique is substantial when it shows a failure in the proposal's own argument, so that the argument loses a step if the critique stands. It names its places and supports itself in one of these ways:

- **Gap.** A step whose premises can hold while its conclusion fails. The critique gives the case and says what the step assumes that the proposal does not say. For `X → Y` it shows that X can be true while Y is false, which exposes the premise the step assumes. This covers every form of argument: deductive, causal, analogical and definitional, and a claim that X is necessary for Y is the step `¬X → ¬Y`.
- **Circularity.** A step that presupposes what it concludes. The critique quotes the premise and the conclusion and shows that they say the same thing.
- **Conflict.** Two places that cannot both hold. The critique quotes both.
- **Shift.** A word that changes meaning between a premise and a conclusion. The critique quotes both uses.
- **Unused text.** A premise no step uses, or a stretch that serves no step. The critique quotes it.

A wish for more, such as a definition, a mechanism, a criterion or a theory the proposal does not offer, is not a critique unless it shows a step that fails without it. The adversary raises every critique it can substantiate, and never raises again what an earlier critique already said: it presses the earlier one instead.

A critique that shows a failure must be accepted, and the acceptance says what the argument now lacks. The advocate does not discuss a critique because the missing premise is true or obvious: if the proposal does not say it, the critique stands. It discusses a critique only on one of two grounds:

- **Misreads.** The critique misreads the proposal. The advocate quotes the words it misreads and, when there is one, the place that answers it.
- **Unclear.** The critique does not make clear what fails: it gives no case, or in its case a premise is false or the conclusion still holds, or it asks for something no step of the argument needs. The advocate says exactly what is missing from it.

Quoting the place a critique attacks does not answer it, because the question is what that place rests on, not whether it is said. The advocate proposes no wording of its own: it accepts a critique or says what is wrong with it, and only the adversary words a critique.

Code checks only the form: a critique has an id no earlier critique used, a kind, at least one place, a point and its support; an answer names its critique, accepts or discusses, and gives reasons, with a ground when it discusses and a quote when the ground is misreads; a response names its critique, drops or presses, and gives reasons. Every quote in an answer or a response is checked like a place. The other side judges the substance under the same discipline, so no third agent referees.

### The debate

The turn order is adversary 1, advocate 1, adversary 2, advocate 2, …, so that the advocate answers every critique.

- **Round 1.** The adversary reads the proposal and raises every critique it can substantiate. The advocate answers each one.
- **Later rounds.** The adversary responds to every critique the advocate discussed in its last turn. It drops the critique, saying why the advocate's reading is right or why it cannot give what the advocate asked for, or it presses the critique with the case or the quote the advocate asked for, not a restatement. A critique the advocate discusses again can be pressed again, so a thread runs until one side yields or the round limit ends it. It then raises any new critique the advocate's answers open up, under the same discipline. The advocate answers every critique pressed or raised.
- **Satisfaction.** The adversary declares itself satisfied when it has nothing to press and nothing to raise, so a satisfied turn drops every critique still under discussion.
- **Completeness.** The orchestrator checks that the advocate answers every critique raised or pressed in the adversary's last turn, once each and no other, and that the adversary responds to every critique the advocate discussed in its last turn, once each and no other. A turn that fails a check gets the failures and one retry, and a second failure fails the run.
- **Quotes are checked.** Every place in a critique and every quote in an answer or a response occurs in the proposal word for word, with whitespace normalised, under the same retry.
- **Ending.** The debate ends at the first adversary turn that declares satisfaction, which needs no advocate turn after it, or after the advocate's turn of the last round. A critique is settled when the advocate accepted it or the adversary dropped it. A critique the advocate discussed that the adversary has not dropped is open: both sides' arguments stand in the report, and neither has yielded.
- **Round limit.** Set in configuration: default 1, range 1–10, and a value outside the range is clamped with a warning so a mistyped setting cannot run without end. With one round the adversary critiques the proposal once and the advocate answers every critique, so every accepted critique is settled and every discussed one is open. The MCP tool has no field for the limit, so every MCP call runs under the configured one. On the command line, `run --rounds` sets the limit of one run.

Every turn sees the proposal, the caller's context and every earlier turn of the debate.

### What the report contains

| Field | Content |
| --- | --- |
| Run | the run id and the location of its record |
| Ending | the adversary satisfied in round r of N, or the round limit reached with open critiques. It describes how the debate ended and is never a verdict. |
| Critique ledger | one row per critique: its kind, its first place, the round that raised it and its final status, which is accepted, dropped or open |
| Accepted critiques | each critique with the advocate's answer, which says what the argument lacks |
| Dropped critiques | each critique with the advocate's answer and the adversary's reason for dropping it, so the caller sees what the proposal answered and where |
| Open critiques | each critique with both sides' arguments |
| Transcript | every turn, headed by round and role |

The report comes in two renderings of the same data: markdown for an agent or a person to read, and JSON for a program.

### Agent prompts

An agent file holds the agent's instructions: its role, the discipline and what to do each turn. The discipline lives in its own file that both prompts include. The orchestrator appends the output shape, the JSON object a turn replies with, to every prompt, because the shape is the orchestrator's protocol and a custom prompt must not be able to break it.

A prompt is looked up in layers: the file named for this run, then the project's `.sedcontra/agents/`, then the user's config directory, then the copy bundled in the binary. The command line names a file with `run --adversary <file>` and `run --advocate <file>`; the MCP tool has no field for a prompt, so every MCP call uses the layered lookup. The run record snapshots the prompts each run used.

### Front ends

Both front ends are thin layers over the same core and write the same run record. One binary serves both: `serve` starts the MCP server and `run` runs one cross-examination.

#### MCP server

- Stdio transport, built on `rmcp`, the official Rust MCP SDK, which implements the 2026-07-28 MCP spec.
- One tool, `videtur_quod`, with two fields:
  - `proposal` (string, required): the text to cross-examine;
  - `context` (string, optional): what the caller needs the proposal for and any evidence it already has.
- The call waits until the report is ready. When the client supplies a progress token, the server sends a progress notification after every turn, so the client sees the run advancing.
- The result carries the markdown report as text content and the JSON report as structured content.
- A failed run returns an error result with the reason and every completed turn.
- A cancelled request stops the run. MCP sends no result for a cancelled request, so the completed turns remain only in the run record.
- A client whose tool-call time limit is shorter than a debate cuts the call off, and such a client should use the command line. `rmcp` also implements MCP's tasks extension, which lets a call return at once and deliver its result later; it is a later option and stays out of the first build.

#### Command line

- `run <proposal file, or - for stdin> [--context <file>] [--rounds <n>] [--adversary <file>] [--advocate <file>] [--format md|json] [--out <file>]`.
- It prints the report to stdout, or writes it to `--out`, and always writes both renderings into the run record.
- It exits with 0 whenever a report was produced, whatever the ending, and with a non-zero code on failure, printing the reason to stderr.
- This is the background form: the caller's harness starts the process in the background and reads the report when it exits. Harnesses such as Claude Code notify the agent when a background process exits.

### Run record

Each run gets one folder under a configured runs directory. It holds the proposal and context, a snapshot of the resolved configuration and agent definitions, every turn as it completes (the raw model output and the parsed data), the status (running, done, failed or cancelled) and the report in both renderings. Because every turn is written when it completes, a later checkpoint-and-resume feature can restart a run from its last turn.

### The core and the swappable parts

Every decision in this spec stays changeable without rewriting the core. The core is built first:

- the data model: proposal, critiques, answers, responses, turns, ledger and report;
- the debate loop, with its turn order, completeness checks, early stop and round limit;
- the quote check.

Every other part sits behind one interface, so it can be replaced, and new parts of the same kind can be added alongside:

| Part | First implementation |
| --- | --- |
| Model provider | a client for OpenAI-format chat APIs, configured for DeepSeek |
| Agent prompts | one Markdown file each for the adversary and the advocate, holding only the prompt |
| Critique discipline | explanatory-critique, in its own file that both prompts include |
| Front ends | MCP server, command line |
| Report renderers | markdown, JSON |

The agents are independent of the provider: a prompt names no provider and no model. The configuration assigns each agent its model in its own `[models.<agent>]` table. The advocate and the adversary both default to `deepseek-flash`, and either can run on a different model or provider without touching the other. The built-in defaults live in a bundled configuration file rather than in code, and a user's file overrides them table by table; it also holds the provider settings, the round limit and the runs directory.

### Model access

- The first provider is DeepSeek, through its OpenAI-format API at `https://api.deepseek.com`. The default model is `deepseek-flash`, the cheap one; `deepseek-v4-pro` is the alternative. The API key comes from a key file that `sedcontra key set <provider>` writes with mode 600, `~/.config/sedcontra/keys/<provider>` by default, and the tool refuses a key file other users can read. The environment variable named in the configuration, `DEEPSEEK_API_KEY` by default, is a fallback for CI, read only when no key file exists.
- The key must never reach a model. It travels only in the HTTP `Authorization` header and never enters a prompt, and every provider error is redacted and shortened before it can reach a report, an MCP result, stderr or the run record.
- The advocate and the adversary return structured data for the orchestrator to check, so every turn requests DeepSeek's JSON output mode, `response_format: {"type": "json_object"}`. The mode requires the prompt to mention JSON and show an example of the structure, which the output shape the orchestrator appends does. It can also return blank content when the model's output was not valid JSON, so the provider retries a blank or unparsable reply twice before the turn fails.
- A reply that reaches `max_tokens` is not a failure. The cut-off reply stays in the agent's history, the agent is told to stop deliberating and give its decision now, and only a second cut-off in the same turn fails the run.
- Both DeepSeek models run in thinking mode. Both agents reason at effort `high` by default, because the advocate judges each critique as the adversary judges each answer. Each agent's depth is set apart from the other's, in its own `[models.<agent>.request]` table: `reasoning_effort` (`low`, `high` or `max`), or `thinking = { type = "disabled" }` to turn reasoning off. These are the provider's own parameters, so they live in the configuration and not in code or prompts.

## Prior art

- The [adversarial agents example in Microsoft's MCP for Beginners](https://github.com/microsoft/mcp-for-beginners/blob/main/05-AdvancedTopics/mcp-adversarial-agents/README.md) debates a proposition between a FOR and an AGAINST agent and lets a third agent, a judge, give a verdict with a confidence score. This design keeps its per-agent histories and drops the judge, the assigned sides and the shared tools, because the report must never read as a verdict and the two sides work out one question together.
- [Elenchus](https://github.com/jhlee0409/elenchus-mcp) tracks every issue in a Verifier–Critic debate through explicit states, which the critique ledger adopts.
- GAAR ([Ryu et al., 2026](https://arxiv.org/abs/2603.17432)) reconstructs arguments with their implicit premises and reduces them to minimal premise sets, which the gap critique's missing premise and the unused-text critique draw on.

## Constraints

- Pure Rust. Every part is Rust code or a Rust crate, so the application uses no Python or Node runtime, no LangChain or LangGraph, and no subprocess: nothing shells out to `claude`, `codex` or any other program.
- The async runtime is tokio, which `rmcp` is built on.
- The MCP server runs over stdio.
- Testing. The build is not test-driven, and code that compiles counts as done. Tests cover only the areas where compiling code can still produce a wrong report without any visible error, and they use a scripted model with no network. Those areas are:
  - the quote check, word for word with whitespace normalised;
  - the completeness checks: every critique answered once, every discussed critique responded to once, no critique id reused, and a satisfied turn that presses or raises nothing;
  - the ending of the debate: the early stop at the first satisfied adversary turn and the split into accepted, dropped and open critiques;
  - the key handling: owner-only storage, the refusal of a key file others can read, and redaction.

## Non-goals

- Judging the proposal. The report describes and the caller decides.
- Anything the caller does with the report.
- A reconstruction of the proposal's argument. The debate examines the proposal's steps where they stand in the text, so the report quotes the proposal and builds no tree of claims.
- Judging the order of the proposal's text.
- Retrieval or evidence gathering by the advocate or the adversary.
- Checkpoint and resume in the first build. The run record prepares for them.
- An evaluation against [Arguinas](https://github.com/GyoukChu/Arguinas) in this build. Its implicit premises and fallacy labels could be matched against accepted critiques later.

## Anti-goals

These count as failure even if everything else works:

- A critique that shows no failure is accepted or stands at the end.
- The advocate words the argument: it proposes a premise, a reading or a repair of its own.
- A critique goes unanswered, a discussed critique gets no response, or an answer or a response stands without reasons.
- A dropped critique reads as one the proposal lost, or an open critique reads as settled either way.
- The report reads as a verdict on the proposal.

## Left to the builder

- Internal structure and naming, the binary's name, the configuration format and the location of the runs directory.
- How each box holds its conversation: whether an actor keeps its own message history across turns or receives the whole debate each turn, provided it receives nothing except what the orchestrator sends.
- The JSON shapes of the adversary's and the advocate's turns, which the orchestrator appends to the prompts.
