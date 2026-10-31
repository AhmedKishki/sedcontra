# Sed Contra: spec

## Why this exists

An agent that writes a proposal draws every connection in it from its own model, so it has nothing outside itself to check those connections against. A connection it invented therefore reads exactly like one that holds. Text without any connection passes just as easily: a sentence that links to nothing, a conclusion announced before its argument, or a "not only X but also Y" where X and Y have no relation reads as fluently as an argument, and these are the marks of AI writing.

The application counters both with an independent cross-examination. One agent, the advocate, reconstructs the proposal's argument as explicit inferences, and a second agent, the adversary, isolated from the first, attacks each inference under a shared standard of critique. Their debate shows the agent that wrote the proposal where it invents a connection, assumes what it concludes, rests on a premise it never states, carries a premise its argument does not need, or carries text the argument does not use.

## What the application is

The application is a box that holds two boxes. The outer box, the orchestrator, takes a proposal from a calling agent, the caller, and runs the cross-examination. Inside it, the advocate reconstructs the proposal's argument as a tree of inferences, and the adversary critiques that tree. They debate under one shared standard, explanatory-critique, until the adversary is satisfied or a round limit is reached, and the orchestrator returns a report to the caller.

The cross-examination sets out the argument the proposal makes and never builds a better one. It breaks that argument down into its inferences and everything they assume or presuppose for their conclusions. Every premise the proposal states is accepted as given, and so is every premise the debate shows an inference to need without the proposal stating it. A premise the argument turns out not to need is pruned, which corrects an argument that assumes more than it needs but still works. An inference is refuted when no premise the proposal could hold would complete it.

The caller is autonomous. What it does with the report lies outside this project, whose product is the analysis, so the report describes the proposal and never rules on it.

The application has one function, `videtur_quod`, which any agent can reach in two ways: as a tool on an MCP server, or as a command-line process that the caller's harness runs in the background.

## Terms

The structure the advocate builds is a directed graph without cycles. This spec calls it a tree, as the owner does.

| Term | Meaning |
| --- | --- |
| Proposal | The text the caller submits, of any length and subject. |
| Claim | A substantive proposition of the argument, stated by the proposal or added during the debate; a node of the tree. |
| Stated claim | A claim the proposal states, with at least one place. |
| Added claim | A claim the advocate adds during the debate because an inference needs it and the proposal does not state it. It has no place, and its origin names the round and the critique that added it. |
| Inference | A connection `P, Q → Z` from one or more premise claims to one conclusion claim; an edge of the tree. |
| Reported inference | An inference the proposal sets out without drawing it in its own voice: it attributes the inference to someone else or sets it out in order to reject it. |
| Conceded inference | An inference the advocate concedes to fail as the proposal draws it, in answer to a gap or circular critique. |
| Place | An exact quote from the proposal that states a claim or an inference. |
| Connective | A word from a configured list, such as "because" or "thus", that marks an inference in the proposal. |
| Root | A claim that no inference concludes: a premise the argument starts from. |
| Leaf | A claim that no inference uses as a premise. |
| Conclusion | A claim the advocate marks as what the proposal argues for. |
| Unsupported conclusion | A conclusion that no inference concludes, apart from reported ones. |
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

1. The advocate reads every paragraph, sentence and phrase and identifies the substantive claims. Wording that carries no claim and no connection yields nothing. Each claim states one assertion and keeps the proposal's negation, modal and quantifier words, so that a paraphrase cannot drop a "not", a "may" or a "some".
2. It identifies the inferences between claims and records them in logical order, whatever order the text uses: "X because Y" becomes `Y → X`. The tree is therefore in logical order by construction, and nothing judges the order of the text itself.
3. It marks the conclusion or conclusions the proposal argues for.
4. It records every place that states each claim, and each place quotes at most one sentence. For each inference it records the place that states the connection, such as the "because", when the text states one. Every connective in the proposal is the place of some inference, since a connective marks an inference the proposal draws.
5. It flags an inference as reported when the proposal does not draw it in its own voice, so that the debate tests the proposal's own argument.
6. It stays faithful to the caller. The first tree records the connections the proposal draws, including weak ones, and adds nothing the proposal did not say. Repair happens only in the debate, where every change traces back to a counter-proposal.

The orchestrator then checks the tree in code, as set out under [The checks](#the-checks), and puts every revised tree the advocate submits during the debate through the same checks. A claim the advocate adds during the debate has no place; it records instead the round and the counter-proposal that added it, and so does an inference first recorded during the debate.

### Explanatory-critique

The advocate and the adversary argue under one standard, and it binds both sides equally. A critique takes one of four kinds.

- **Gap.** A critique of an inference names concretely what the inference misses and shows a case in which its premises hold and its conclusion fails. For `X → Y` it shows that X can be true while Y is false, typically as `X ∧ ¬Q → ¬Y`, which exposes Q as a premise the inference assumes. The standard covers every form of argument: deductive, causal, analogical and definitional. A claim that X is necessary for Y is the inference `¬X → ¬Y` and is critiqued the same way. An insufficient or inconclusive inference, a hidden premise and an invented connection are each shown by such a case. What a conclusion presupposes is exposed the same way, since a case in which the presupposition fails is a case in which the conclusion fails.
- **Superfluous premise.** A critique of a premise shows that the inference does not need it, by deriving the inference's conclusion from its other premises. A counter-case cannot expose such a premise, because the inference holds with or without it. Accepting the critique removes the premise from the inference; if no other inference uses it, the stub finding then reports it for pruning. The inference must keep a stated premise, so a superfluous critique cannot reduce it to premises the advocate added.
- **Circular premise.** A critique of a premise shows that the premise asserts the inference's conclusion, wholly or in part, so the inference assumes what it concludes. Against an added claim it exposes a patch that restates the conclusion, which the advocate then removes; against a stated claim it shows that the proposal argues in a circle. A conditional whose consequent is the conclusion is not circular on that ground, because it often states the very connection a gap critique exposed.
- **Reading.** A critique of the advocate's reading quotes the place the advocate misread and gives the reading the text supports. The misreading can lie in a claim, in an inference the tree records or leaves out, or in a reported flag. An added claim that contradicts the proposal is a misreading too, which the critique shows by quoting the place it contradicts.

Two kinds of inference take only reading critiques: a reported inference, which is not part of the proposal's own argument, and a conceded inference, which the debate has already shown to fail.

An objection without its named target and its case, derivation or reading does not count. Code decides what it can decide without judging meaning, as set out under [The checks](#the-checks), and the other side judges the substance under the same rules, so no third agent referees.

### The debate

The turn order is advocate 1, adversary 1, advocate 2, adversary 2, … so that the adversary closes every round.

- **Round 1.** The advocate builds the tree. The adversary critiques it with counter-proposals, or declares itself satisfied.
- **Later rounds.** The advocate answers every counter-proposal from the adversary's last turn and submits the revised tree. The adversary then responds.
- **The advocate's answers.** The advocate answers each counter-proposal in one of four ways and gives its reasons.
  - **Accept.** It changes the tree as the critique says: it adds the missing premise for a gap, removes the premise for a superfluous critique, removes the added premise for a circular critique, and corrects the reading for a reading critique.
  - **Adapt.** It changes the tree differently, in a way that answers the critique.
  - **Concede.** It grants that a gap or circular critique refutes the inference as the proposal draws it. It concedes when no premise the proposal could hold closes the gap, because any such premise would contradict the proposal or assert the conclusion, or when a stated premise asserts the conclusion. The inference stays in the tree, marked as conceded, so that the report shows the refutation; an added premise that a conceded circular critique names leaves it.
  - **Reject.** It shows that the critique fails, on one of three grounds: the proposal states the missing premise at a quoted place, the premise follows from claims the proposal makes, or the counter-proposal's own premises can hold while its conclusion fails. A rejection on the first two grounds names the claims that supply the premise, and the revised tree carries them as premises of the inference, so that the report can show how the conclusion depends on them. The last ground also answers a superfluous or circular critique: a case where the remaining premises hold and the conclusion fails shows that the premise is needed, and a case where the premise holds and the conclusion fails shows that the premise does not assert the conclusion.
- **True is not stated.** The advocate may not reject a critique because the missing premise is true. The argument still rests on a premise the proposal never states, and exposing such premises is the report's purpose. The advocate accepts the critique and adds the premise, marking it as backed by the caller's context when the context supports it.
- **The adversary's response.** The adversary drops each rejected critique or maintains it with an answer to the advocate's reasons, checks each adaptation, and critiques the revised tree afresh. A conceded critique is settled and takes no response. The adversary declares itself satisfied when it can show no critique that meets the standard.
- **What code finds.** Each adversary turn lists the cases code found for the adversary to judge, as set out under [The checks](#the-checks).
- **Ending.** The debate ends at the first adversary turn that declares satisfaction, or when the round limit is reached. When the adversary is not satisfied in its last turn, its critiques fall into two groups. A critique it maintained against the advocate's rejection has been argued by both sides and is an open critique. A critique new in that turn never reached the advocate, so the report labels it unanswered.
- **Round limit.** Set in configuration: default 1, range 1–10, and a value outside the range is clamped with a warning so a mistyped setting cannot run without end. With one round the adversary critiques the advocate's first tree once, and every critique reaches the caller unanswered. The MCP tool has no field for the limit, so every MCP call runs under the configured one. On the command line, `run --rounds` sets the limit of one run, and the evaluation sets its own limit of 10 through `eval --rounds`.

Every turn sees the proposal, the caller's context and every earlier turn of the debate.

### The checks

Code decides whatever it can decide without judging meaning, so that neither agent has to notice it. Where finding a case needs no meaning but judging it does, code finds the case, the orchestrator lists it in the adversary's next message, and the report shows it. The agents judge only what meaning settles, such as whether a case defeats an inference, a derivation holds or a reading is right.

Every tree the advocate submits passes these checks:

- every inference refers to existing claims and has at least one premise, and at least one conclusion is marked;
- no path loops back on itself;
- every place occurs in the proposal word for word, with whitespace normalised, and the orchestrator records where;
- every claim has a place, or an origin as an added claim, and the first tree has no added claims and no origins;
- a claim's place quotes at most one sentence: it crosses no sentence end, paragraph break or start of a list item, where a sentence end is a ".", "!" or "?" followed by whitespace and a capital letter;
- every occurrence of a connective in the proposal lies within the place of an inference;
- no premise's place overlaps a place of its inference's conclusion;
- a stated claim's text contains every word from the configured list of negation, modal and quantifier words that its places contain;
- every inference has at least one stated premise.

The orchestrator compares each revised tree with the previous one, so that every change traces back to a critique answered in the same turn. A critique *touches* an inference when it targets the inference, when it is a reading critique on the inference or on one of its claims, or when the inference is new and names the critique as its origin.

- An inference leaves the tree, or changes its conclusion, only when an accepted or adapted critique touches it, and never in the turn it is conceded.
- A stated premise leaves an inference that stays in the tree only through a superfluous critique that names it or a reading critique on the premise or the inference, accepted or adapted in the turn.
- An added premise leaves an inference only when a critique accepted, adapted or conceded in the turn touches the inference.
- A stated claim leaves the tree only through an accepted or adapted reading critique on it, so a pruned premise stays in the tree, where the stub finding reports it.
- An inference's reported flag changes only through an accepted or adapted reading critique on it.
- Every newly added claim and every new inference names as its origin the current round and a critique answered in that turn, and a newly added claim is a premise of an inference that its origin critique touches.

The advocate's answers pass these checks:

- the advocate answers every critique that the adversary raised or maintained in its last turn, once, and no other;
- every answer gives reasons, every rejection names its ground, and every quote occurs in the proposal word for word;
- only a gap or circular critique can be conceded, and a circular critique can be accepted only against an added premise;
- accepting a gap critique gives the inference an added premise whose origin is that critique;
- accepting a superfluous or circular critique removes the named premise from the inference;
- accepting a reading critique changes the claim or inference it targets, or an inference that the claim belongs to;
- adapting a critique changes its target, an inference it touches, or a claim of such an inference;
- conceding keeps the inference in the tree and removes an added premise that a conceded circular critique names;
- rejecting a gap critique on the stated or follows ground names stated claims that the revised tree carries as premises of the inference, and every quote of a stated rejection lies within a place of a named claim.

Every adversary turn passes these checks:

- every critique has an id no earlier critique used and names its kind, its target, its point and its support;
- a gap, superfluous or circular critique targets an inference that is neither reported nor conceded, and a superfluous or circular critique names one of that inference's premises;
- a superfluous critique targets an inference with at least two premises and leaves it a stated premise;
- a reading critique quotes at least one place, and every quote occurs in the proposal word for word;
- the adversary responds to every rejection and to no conceded critique, and gives reasons for every critique it maintains;
- the adversary declares itself satisfied exactly when its turn raises and maintains nothing.

A turn that fails a check gets the list of failures and one retry, and a second failure fails the run. Each failure names what broke the check and what would satisfy it, so the fix on retry is mechanical.

Code finds these cases for the adversary to judge:

- an unsupported conclusion, as a candidate for a connection the tree leaves out;
- a circular candidate: a premise that holds at least a configured share, 0.8 by default, of its conclusion's content words, which are the words outside a configured list of stopwords. It stays a candidate and is never refused, because a conditional whose consequent is the conclusion holds the conclusion's words by nature, and telling such a conditional from a restatement depends on its logical form, which only meaning settles;
- a conceded inference, which takes only reading critiques;
- a critique dropped after a rejection on the stated or follows ground, with the claims that closed it.

The connectives, the qualifier words, the share and the stopwords live in the configuration, so they can follow the language of the proposal.

### What the report contains

| Field | Content |
| --- | --- |
| Run | the run id and the location of its record |
| Ending | the adversary satisfied in round r of N, or the round limit reached with open or unanswered critiques, followed by the number of unsupported conclusions and of conceded inferences. It describes how the debate ended and is never a verdict. |
| Conclusions | for each conclusion: the added claims its support rests on, the conceded inferences and the open or unanswered critiques its support passes through, and whether some line of support avoids every conceded inference |
| Critique ledger | one row per critique: its kind, its target, the round that raised it and its final status, which is premise added, premise removed, patch removed, reading corrected, adapted, conceded, dropped after rejection with the ground, open or unanswered |
| Final tree | claims, inferences and conclusions, each marked with its places in the proposal or with the round and counter-proposal that added it, and each reported or conceded inference marked as such |
| Added claims | every premise the proposal needs and does not state, with the inference it completes and the critique that exposed it |
| Stated premises that close a gap | every claim a rejection on the stated or follows ground put into an inference, beside the counter-case that showed the conclusion depends on it |
| Conceded inferences | every refuted inference, with the critique the advocate conceded |
| Changes | every accepted or adapted counter-proposal and what it changed, which code computes by comparing the tree before and after the answer: the claims and inferences it added and the premises and inferences it removed. An added claim is a premise the proposal needed and did not state; a removed premise is one it did not need. |
| Open critiques | when the limit was reached: the critiques the adversary maintained against the advocate's rejection, each with both sides' arguments |
| Unanswered critiques | when the limit was reached: the critiques new in the adversary's last turn, listed apart because the advocate had no turn to examine them |
| Structural findings | see below |
| Transcript | every turn, headed by round and role |

A line of support for a conclusion is a set of inferences, none of them reported, that derives the conclusion from claims no such inference concludes. Code computes the Conclusions field from the final tree and the ledger, so the caller sees which conclusions a critique or a concession reaches without tracing the tree itself.

The report comes in three renderings of the same data: markdown for an agent or a person to read, JSON for a program, and Argdown for the final tree. Argdown is an established syntax for premise–conclusion structures, so the tree also opens as an argument map in Argdown's tools, and the markdown report shows its final tree in the same syntax.

### Structural findings

Each finding is computed in code by an analyzer that reads the final tree and the proposal, without a model call. They run on the final tree, so any misreading the adversary caught is already corrected.

- **Restatement.** The report quotes every place of the claim and suggests keeping one. A conclusion announced before its argument shows up here, because the advocate records the announcement and the final statement as one claim.
- **Uncovered text.** The report quotes each stretch of the proposal that no place covers as a candidate for cutting. Stretches shorter than a configured minimum, 4 words by default, are ignored.
- **Stub.** The report lists each stub with its claims and places. A stub no longer than a configured threshold, 2 claims by default, is marked as a prune suggestion; a longer one is reported as a separate line of argument.
- **Unsupported conclusion.** The report lists each unsupported conclusion with its places. The proposal asserts it without an argument the tree records, either from the start or because the debate removed the inference that concluded it.
- **Circular candidate.** The report lists each circular candidate in the final tree with its share of the conclusion's content words, as a premise to check for restating its conclusion.

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
- A client whose tool-call time limit is shorter than a debate cuts the call off, and such a client should use the command line. `rmcp` also implements MCP's tasks extension, which lets a call return at once and deliver its result later, but the server does not use it.

#### Command line

- `run <proposal file, or - for stdin> [--context <file>] [--rounds <n>] [--format md|json|argdown] [--out <file>]`. The `argdown` format prints the final tree only.
- It prints the report to stdout, or writes it to `--out`, and always writes every rendering into the run record.
- It exits with 0 whenever a report was produced, whatever the ending, and with a non-zero code on failure, printing the reason to stderr.
- This is the background form: the caller's harness starts the process in the background and reads the report when it exits. Harnesses such as Claude Code notify the agent when a background process exits.

### Run record

Each run gets one folder under a configured runs directory. It holds the proposal and context, a snapshot of the resolved configuration and agent definitions, every turn as it completes (the raw model output and the parsed data), the status (running, done, failed or cancelled) and the report in every rendering. Because every turn is written when it completes, a checkpoint-and-resume feature could restart a run from its last turn.

### The core and the swappable parts

Every decision in this spec can change without a rewrite of the core, which holds:

- the data model: proposal, claims, inferences, places, tree, counter-proposals, answers, turns and report;
- the debate loop, with its turn order, completeness checks, early stop and round limit;
- the checks.

Every other part sits behind one interface, so it can be replaced, and new parts of the same kind can be added alongside:

| Part | Implementation |
| --- | --- |
| Model provider | a client for OpenAI-format chat APIs, configured for DeepSeek |
| Agent prompts | one Markdown file each for the advocate and the adversary, holding only the prompt |
| Critique standard | explanatory-critique, in its own file that both prompts include |
| Analyzers | restatement, uncovered text, stub, unsupported conclusion, circular candidate |
| Front ends | MCP server, command line |
| Report renderers | markdown, JSON, Argdown |

Agent definitions are looked up in layers, as in [pi-interactive-subagents](https://github.com/amosblomqvist/pi-interactive-subagents): a project's file overrides the user's, and the user's overrides the default compiled into the binary. The agents are independent of the provider: a prompt names no provider and no model. The configuration assigns each agent its model in its own `[models.<agent>]` table. The advocate and the adversary both default to `deepseek-flash`, and either can run on a different model or provider without touching the other. The built-in defaults live in a bundled configuration file rather than in code, and a user's file overrides them table by table; it also holds the provider settings, the round limit, the lists and thresholds that the checks and analyzers use, and the runs directory.

### Model access

- The default provider is DeepSeek, through its OpenAI-format API at `https://api.deepseek.com`. The default model is `deepseek-flash`, the cheap one, since the owner prefers a cheap model; `deepseek-v4-pro` is the alternative. The API key comes from a key file that `sedcontra key set <provider>` writes with mode 600, `~/.config/sedcontra/keys/<provider>` by default, and the tool refuses a key file other users can read. The environment variable named in the configuration, `DEEPSEEK_API_KEY` by default, is a fallback for CI, read only when no key file exists.
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
  - the checks, especially loop detection, the word-for-word quote check and the comparison of a revised tree with the previous one;
  - the analyzers and the support of each conclusion, on hand-built trees;
  - the ending of the debate: the early stop at the first satisfied adversary turn, the split into open and unanswered critiques, and the status each critique ends with;
  - the key handling: owner-only storage, the refusal of a key file others can read, and redaction.

## Non-goals

- Judging the proposal. The report describes and the caller decides.
- Disputing a premise. Every premise the proposal states, and every premise its inferences need without stating it, is accepted as given; the report shows what the argument rests on, and the caller weighs it.
- Anything the caller does with the report.
- Judging the order of the proposal's text. The tree is rebuilt in logical order, and the restatement finding catches a conclusion stated before its argument.
- A separate prune function. Prune suggestions are part of the report.
- Retrieval or evidence gathering by the advocate or the adversary.
- Checkpoint and resume. The run record prepares for them.

## Anti-goals

These count as failure even if everything else works:

- The advocate's first tree repairs or extends the proposal, so the findings describe the advocate's argument instead of the proposal's.
- An objection that does not meet explanatory-critique changes the tree or stands at the end.
- An unanswered critique reads as one that survived the advocate's answer.
- A counter-proposal goes unanswered, or a rejection stands without reasons.
- A conclusion loses its support, or an inference is refuted, without the report saying so.
- A patch restates the conclusion it supports, or replaces the premises the proposal states.
- The report reads as a verdict on the proposal.

## Left to the builder

- Internal structure and naming, the binary's name, the configuration format and the location of the runs directory.
- How each box holds its conversation: whether an actor keeps its own message history across turns or receives the whole debate each turn, provided it receives nothing except what the orchestrator sends.
- The JSON shapes of the advocate's and the adversary's turns.
