<h1 align="center"><img src="docs/logo.svg" alt="Sed Contra" width="640"></h1>

## Videtur quod … sed contra

Each article of Thomas Aquinas's *Summa Theologiae* opens with a position that seems to hold, introduced by the words *videtur quod*, "it seems that". Two words then turn the article against it: *sed contra*, "but on the contrary".

The written form comes from the disputations of the medieval universities. There a master set a question, opponents raised objections, and a respondent, usually an advanced student, answered them on the spot. At a later session the master gave his determination, the answer that settled the question.

Sed Contra stages that disputation over an agent's proposal. The proposal is what seems to be so, and the tool that receives it is therefore named `videtur_quod`. Its adversary raises the objections and its advocate answers them, as opponent and respondent did. The master's determination is the one part it leaves out: the agent that called the tool decides what the report means for its proposal, so the report describes the proposal and never rules on it.

## The principle

An agent cannot cross-examine its own proposal, because it drew every connection in the proposal from its own model. An invented connection therefore reads to it exactly like one that holds. Text that connects to nothing passes just as easily, since it reads as fluently as an argument. Sed Contra supplies the outside check: two model agents, independent of the agent that wrote the proposal, test it for connections it invented, premises it never states, premises it does not need and text its argument never uses.

The two agents argue under one standard, explanatory-critique, which counts a critique only when it shows its point:

- a **gap**, by a case in which an inference's premises hold and its conclusion fails;
- a **superfluous premise**, by deriving the inference's conclusion without it;
- a **misreading**, by quoting the passage the advocate misread.

The debate therefore turns on cases, derivations and quotes that the other side can check. The advocate's rejections must meet the same standard, so each rests on a quoted passage of the proposal, a derivation from the proposal's claims or a counter-case of its own. Truth is not one of these grounds: a missing premise that is true is still a premise the proposal never states, so the advocate adds it to the tree.

## Architecture

```mermaid
flowchart TB
    caller(["Calling agent"])
    subgraph frontends ["Front ends"]
        mcp["MCP tool videtur_quod<br>(serve)"]
        cli["Command line<br>(run)"]
    end
    subgraph orchestrator ["Orchestrator"]
        debate["Debate loop"]
        checks["Checks"]
        analyzers["Analyzers"]
        report["Report<br>markdown · JSON · Argdown"]
    end
    advocate["Advocate"]
    adversary["Adversary"]
    provider["Model provider<br>OpenAI-format chat API"]
    record[("Run record")]

    caller -->|"proposal, context"| frontends
    frontends --> debate
    debate <-->|"turns"| advocate
    debate <-->|"turns"| adversary
    advocate & adversary --> provider
    debate -->|"every tree and turn"| checks
    debate -->|"final tree"| analyzers
    debate -->|"outcome"| report
    analyzers -->|"findings"| report
    debate -->|"every turn"| record
    report --> record
    report --> frontends
    frontends -->|"report"| caller
```

Each part of the diagram lives in its own module:

| Part | Module | Job |
| --- | --- | --- |
| Front ends | `src/mcp.rs`, `src/main.rs` | Take the proposal and its context from the caller and return the report, as an MCP tool over stdio or as a command. |
| Orchestrator | `src/run.rs`, `src/debate.rs` | Passes each turn between the advocate and the adversary and ends the debate. |
| Advocate, adversary | `src/actor.rs`, `agents/` | Each runs as its own task with its own history and no tools, so it sees only what the orchestrator sends it. |
| Checks | `src/check/` | Reject a malformed tree or turn before the debate goes on. |
| Model provider | `src/provider/` | Sends each agent's requests to the provider its configuration names, DeepSeek by default. |
| Analyzers | `src/analyze/` | Compute the findings from the final tree without a model call. |
| Report | `src/report/` | Builds the report from the debate's outcome and the findings, and renders it as markdown, JSON and Argdown. |
| Run record | `src/record.rs` | Writes one folder per run with the inputs, the resolved settings, every turn as it completes, and the report. |
| Evaluation | `src/eval.rs` | Runs the orchestrator on each record of the Arguinas test split, then has a judge agent score each report against the record's reference reconstruction. |

A run goes through five steps:

1. **Reconstruction.** The advocate rebuilds the proposal's argument as a tree whose nodes are claims and whose edges are inferences `P, Q → Z`. Every claim carries the exact quotes that state it. The first tree repairs nothing, so every later change traces back to a critique.
2. **Checks.** Code rejects a turn when:
   - its tree has a loop, a reference to an unknown claim, or no marked conclusion;
   - it quotes text that does not occur in the proposal word for word;
   - a critique or an answer in it lacks its required parts;
   - it leaves a critique unanswered.

   A rejected turn gets one retry with the failures listed; if the retry fails too, the run fails.
3. **Debate.** The adversary critiques the tree under explanatory-critique. The advocate accepts, adapts or rejects each critique, revising the tree for each one it accepts or adapts. The adversary then drops or maintains each rejected critique before it critiques the revised tree. The debate ends when the adversary is satisfied or the round limit is reached.
4. **Findings.** The analyzers compare the final tree with the proposal to find restatements, stretches of text no quote covers, and stubs: claims from which no inference reaches a conclusion.
5. **Report.** The report states how the debate ended and holds a ledger of every critique with its final status, the final tree in Argdown, the changes the debate forced, the critiques left open or unanswered, the findings and the full transcript.

[SPEC.md](SPEC.md) holds the full design, including the rules of the debate and every field of the report.

## Installation

Prerequisites:

- Rust 1.95 or later;
- an API key for DeepSeek or another provider with an OpenAI-format chat API.

The paths below are those on Linux.

1. Install the binary into `~/.cargo/bin`, which rustup puts on your PATH:

   ```sh
   cargo install --path .
   ```

   Run the same command after pulling changes to update the installed binary.

2. Store the provider's key in your own terminal, not through an agent, so that the key never passes through an agent's context:

   ```sh
   sedcontra key set deepseek
   ```

   The command reads the key without echoing it and stores it in `~/.config/sedcontra/keys/deepseek`, a file only you can read. The environment variable `DEEPSEEK_API_KEY` is a fallback for CI, read only when no key file exists. Do not export it in a shell profile or pass it to an MCP client with `-e`: an agent with a shell can print the first, and an agent with file tools can read the second from the client's configuration.

3. Deny agents the key directory, because an agent that runs commands as you can read any file you can. In Claude Code, add this rule to `~/.claude/settings.json`:

   ```json
   { "permissions": { "deny": ["Read(~/.config/sedcontra/keys/**)"] } }
   ```

   The rule covers Claude Code's file tools and the file commands it recognises in Bash, such as `cat`, but not a script that opens the file itself. [Claude Code's sandbox](https://code.claude.com/docs/en/sandboxing) blocks that script too, because it restricts every process.

4. Register the MCP server, here with Claude Code:

   ```sh
   claude mcp add sedcontra -- "$(command -v sedcontra)" serve
   ```

   A client whose tool calls time out before a debate ends can instead run the command line in the background. It prints the report to stdout as `md`, `json` or `argdown`:

   ```sh
   sedcontra run proposal.md --context context.md --format md
   ```

### Configuration

The defaults are in [config/default.toml](config/default.toml). A `sedcontra.toml` in the working directory, or else `~/.config/sedcontra/config.toml`, overrides any of its values, table by table.

- To run an agent on another provider, add the provider under `[providers]`, store its key as in step 2, and name the provider in that agent's `[models.<agent>]` table.
- To change a prompt without rebuilding, put a file with the same name as one in [agents/](agents/) into `.sedcontra/agents/` or `~/.config/sedcontra/agents/`.

## References

- Hyun Ryu, Gyouk Chu, Gregor Betz, Eunho Yang, Carolyn Rose and Sean Welleck. *Argument Reconstruction as Supervision for Critical Thinking in LLMs*. [arXiv:2603.17432](https://arxiv.org/abs/2603.17432), 2026. It introduces the GAAR reconstruction engine, whose reduction to minimal premise sets the superfluous-premise critique adopts, and the Arguinas dataset.
- [Arguinas](https://github.com/GyoukChu/Arguinas): the code and data whose test split the evaluation scores against. The dataset states no license, so download it for local use and keep it out of the repository.
- [Argdown](https://argdown.org): the syntax of the final tree, which opens as an argument map in Argdown's tools.
- [Adversarial agents example in Microsoft's MCP for Beginners](https://github.com/microsoft/mcp-for-beginners/blob/main/05-AdvancedTopics/mcp-adversarial-agents/README.md): the source of the per-agent histories.
- [Elenchus](https://github.com/jhlee0409/elenchus-mcp): tracks every issue in a debate through explicit states, which the critique ledger adopts.
