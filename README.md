<h1 align="center"><img src="docs/logo.svg" alt="Sed Contra" width="640"></h1>

An agent builds the argument, the other tries to break it.

## The idea

An agent can't really review its own proposal. Every step in the argument came out of the same model that would be doing the checking, so a step it made up looks just as solid to it as one that's true. Filler gets through the same way, because a sentence that supports nothing still reads fine.

Sed Contra gets that review from outside. The advocate rebuilds the proposal as a tree of claims and quotes the proposal's own words for each one, and code checks that every quote really is in the text. The adversary then goes looking for holes. If there's another round, the advocate answers each critique and fixes the tree where the critique is right, and the adversary gets to respond.

A critique has to prove its point. "This seems weak" doesn't count. It has to do one of four things:

- show a gap, with a case where the premises are true and the conclusion is still false;
- show that a premise isn't needed, by reaching the conclusion without it;
- show that a premise just restates the conclusion;
- show a misreading, by quoting the passage the advocate got wrong.

Say a proposal argues "our queries are slow, so we should move to Postgres". The adversary can answer that the queries might be slow because an index is missing, and then moving to Postgres fixes nothing. That's a gap: the proposal assumed, without saying so, that the database was the problem.

The advocate plays by the same rules when it pushes back, so its answer needs a quote, a derivation or a counter-case of its own. It can't wave off a missing premise because the premise happens to be true. If the proposal leaned on something it never said, that goes into the tree.

The report lists every critique and how it ended, what the tree had to change, what's still open, and the text the argument never used. It doesn't say whether the proposal is good. That's for the agent that asked.

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

A run has five steps:

1. **Reconstruction.** The first tree has claims as its nodes and inferences `P, Q → Z` as its edges, and every claim carries the exact quotes that state it. The advocate records the proposal as written, weak links included, so every later change can be traced to a critique.
2. **Checks.** Code rejects a turn when:

   - its tree has a loop, a reference to an unknown claim or no marked conclusion, or breaks another rule in [SPEC.md](SPEC.md#the-checks);
   - it quotes text that isn't in the proposal word for word;
   - a critique or an answer in it is missing a required part;
   - it leaves a critique unanswered.

   A rejected turn gets one retry with the failures listed. If the retry fails too, the run fails.
3. **Debate.** The adversary critiques the tree under the rules above. The advocate accepts, adapts, concedes or rejects each critique and revises the tree for the ones it accepts or adapts. The adversary drops each rejected critique or keeps pressing it, then critiques the revised tree. This goes on until the adversary is satisfied or the round limit is reached.
4. **Findings.** The analyzers compare the final tree with the proposal. They flag claims the proposal states more than once, stretches of text no quote covers, and stubs: claims that never lead to a conclusion.
5. **Report.** The report says how the debate ended. It holds a ledger of every critique with its final status, the final tree in Argdown, the changes the debate forced, the inferences conceded to fail, the critiques left open or unanswered, the findings and the full transcript.

The full design, including the debate rules and every field of the report, is in [SPEC.md](SPEC.md).

## Installation

You need:

- Rust 1.95 or later;
- an API key for DeepSeek or another provider with an OpenAI-format chat API.

Paths below are for Linux.

1. Install the binary into `~/.cargo/bin`, which rustup puts on your PATH:

   ```sh
   cargo install --path .
   ```

   Run it again after you pull to update the installed binary.
2. Store the provider's key yourself, in your own terminal.

   ```sh
   sedcontra key set deepseek
   ```

   The command reads the key without echoing it and stores it in `~/.config/sedcontra/keys/deepseek`, a file only you can read.
3. Register the MCP server. With Claude Code:

   ```sh
   claude mcp add sedcontra -- "$(command -v sedcontra)" serve
   ```

   A debate can take several minutes. If your client's tool calls time out before it finishes, run the command line in the background instead. It prints the report to stdout as `md`, `json` or `argdown`:

   ```sh
   sedcontra run proposal.md --context context.md --format md
   ```

### Configuration

Defaults live in [config/default.toml](config/default.toml). Sed Contra reads `sedcontra.toml` from the working directory if there is one, otherwise `~/.config/sedcontra/config.toml`, and lays it over the defaults table by table.

- To run an agent on another provider, add the provider under `[providers]`, store its key as in step 2, and name the provider in that agent's `[models.<agent>]` table.
- To change a prompt without rebuilding, copy its file from [agents/](agents/) into `.sedcontra/agents/` or `~/.config/sedcontra/agents/` and edit the copy.

## References

- Hyun Ryu, Gyouk Chu, Gregor Betz, Eunho Yang, Carolyn Rose and Sean Welleck. *Argument Reconstruction as Supervision for Critical Thinking in LLMs*. [arXiv:2603.17432](https://arxiv.org/abs/2603.17432), 2026. The paper behind the GAAR reconstruction engine and the Arguinas dataset. The unneeded-premise critique comes from GAAR's reduction to minimal premise sets.
- [Arguinas](https://github.com/GyoukChu/Arguinas): code and data. `sedcontra eval` scores against its test split. The dataset has no license, so download it for local use and keep it out of this repository.
- [Argdown](https://argdown.org): the syntax the final tree is written in, so it opens as an argument map in Argdown's tools.
- [Adversarial agents example in Microsoft&#39;s MCP for Beginners](https://github.com/microsoft/mcp-for-beginners/blob/main/05-AdvancedTopics/mcp-adversarial-agents/README.md): where the idea of giving each agent its own history comes from.
- [Elenchus](https://github.com/jhlee0409/elenchus-mcp): tracks each issue in a debate through explicit states, and the critique ledger borrows that approach.
