# cross-examine

`cross-examine` tests an agent's proposal for connections it invented, premises it never states, premises it does not need, and text its argument never uses. Two model agents, independent of the agent that wrote the proposal, cross-examine it while code checks every step they take. The calling agent receives a report that describes the proposal; the tool never rules on it, because what the caller does with the report is its own decision.

The tool runs as an MCP server or as a command-line process.

## How it works

1. **Reconstruction.** The advocate rebuilds the proposal's argument as a tree. Each substantive claim becomes a node and each inference `P, Q → Z` an edge, recorded in logical order whatever order the text uses: "X because Y" becomes `Y → X`. Every claim carries the exact quotes that state it. The first tree stays faithful to the proposal and repairs nothing, so every later change traces back to a critique.
2. **Checks.** Code rejects a tree with a loop, a reference to an unknown claim, no marked conclusion, or a quote that does not occur in the proposal word for word. A turn that fails gets one retry with the failures listed.
3. **Debate.** The adversary critiques the tree under explanatory-critique, a standard that binds both sides. A critique takes one of three kinds:
   - *gap*: a case in which an inference's premises hold and its conclusion fails. For `X → Y` this is typically `X ∧ ¬Q → ¬Y`, which exposes Q as a premise the inference assumes;
   - *superfluous*: a derivation of an inference's conclusion without one of its premises;
   - *reading*: a quote the advocate misread, with the reading the text supports.

   The advocate answers every critique by accepting, adapting or rejecting it, and a rejection must meet the same standard. A premise that is true but never stated still counts as missing. The adversary then drops or maintains each rejected critique and tests the revised tree. The debate ends when the adversary is satisfied or the round limit is reached.
4. **Findings.** Code compares the final tree with the proposal and reports three kinds of finding:
   - *restatements*: claims stated more than once, which includes a conclusion announced before its argument;
   - *uncovered text*: stretches no quote covers;
   - *stubs*: claims from which no inference reaches a conclusion. Short stubs are suggested for pruning.
5. **Report.** The report has these sections:
   - how the debate ended;
   - a ledger with every critique and its final status;
   - the final tree in [Argdown](https://argdown.org), with the places that state each claim;
   - every change the debate forced;
   - the critiques left open or unanswered;
   - the findings;
   - the full transcript.

   Each run also leaves a record folder holding its inputs, resolved settings, turns and report.

The advocate and the adversary are separate actors: each keeps its own context, has no tools, and sees only what the orchestrator passes it. [SPEC.md](SPEC.md) holds the full design.

## Use

Building needs Rust 1.95 or later, and running needs an API key for DeepSeek or another provider with an OpenAI-format API.

```sh
cargo build --release

# Store the key once, in your own terminal; the input is hidden
target/release/cross-examine key set deepseek

# Command line: the report goes to stdout (md, json or argdown)
target/release/cross-examine run proposal.md --context context.md --format md

# MCP server over stdio, here registered with Claude Code
claude mcp add cross-examine -- "$PWD/target/release/cross-examine" serve

# Evaluation against a local copy of the Arguinas test split
target/release/cross-examine eval 260331-Arguinas-Test-v2.json --limit 20
```

`run` exits with 0 whenever it produces a report, so a harness can start it in the background and read the report when the process exits. Over MCP the call waits until the report is ready, and it sends a progress notification after every turn.

## API key

The key travels only in the HTTP `Authorization` header to the provider and never enters a prompt. Every provider error is redacted before it can reach a report, an MCP result or the run record, so even a provider that echoes the key back passes nothing on to a model.

`cross-examine key set <provider>` stores the key in `~/.config/cross-examine/keys/<provider>` on Linux, a file only you can read (mode 600) in a directory only you can open. The tool refuses a key file that other users can read, and `api_key_file` in the provider's configuration moves the file elsewhere. Run the command in your own terminal rather than through an agent, so the key never passes through an agent's context.

The environment variable named by `api_key_env` (`DEEPSEEK_API_KEY` for the default provider) is read only when no key file exists, and it is meant for CI. Exporting it in a shell profile lets any agent with a shell print it, and passing it to an MCP client with `-e` writes it into the client's configuration, where an agent with file tools can read it.

No file on your account is hidden from an agent that runs commands as you, so also deny the agent access to the key directory. In Claude Code, add a deny rule to `~/.claude/settings.json`:

```json
{ "permissions": { "deny": ["Read(~/.config/cross-examine/keys/**)"] } }
```

The rule covers Claude Code's file tools and the file commands it recognises in Bash, such as `cat`, but not a script that opens the file itself; [Claude Code's sandbox](https://code.claude.com/docs/en/sandboxing) blocks every process.

## Configuration

Settings are read from `cross-examine.toml` in the working directory, or else from the user config directory (`~/.config/cross-examine/config.toml` on Linux):

```toml
rounds = 2                # the round limit, 1–3
runs_dir = "runs"         # default: ~/.local/share/cross-examine/runs on Linux

[providers.deepseek]
base_url = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"  # CI fallback, read only when no key file exists
# api_key_file = "/path/to/key"   # default: ~/.config/cross-examine/keys/deepseek

[analyzers]
stub_max_len = 2          # stubs up to this many claims are suggested for pruning
uncovered_min_words = 4   # shorter uncovered stretches are ignored
```

The prompts live in [agents/](agents/): `advocate.md`, `adversary.md`, `critique-standard.md` and `judge.md`. Each opens with TOML front matter that sets the provider, the model and `max_tokens`. A file with the same name in `.cross-examine/agents/` or `~/.config/cross-examine/agents/` overrides the bundled one, so a model or the critique standard can change without a rebuild.

## Evaluation

`eval` runs each Arguinas record as a proposal. A judge agent then matches the report against the record's reference reconstruction on five measures:

- whether the conclusion was found;
- explicit premises present in the first tree;
- implicit premises present in the first tree, where lower is better because the first tree must not repair the argument;
- implicit premises recovered during the debate;
- fallacies identified.

The reference reconstructions are themselves model output, so the scores measure agreement with GAAR rather than correctness. The dataset states no license, so download it for local use and keep it out of the repository.

## Status

Tests with a scripted model cover the tree checks, the analyzers and the ending of the debate, and tests of the key handling cover its storage and redaction. The tool has not yet run against a live model.

## References

- Hyun Ryu, Gyouk Chu, Gregor Betz, Eunho Yang, Carolyn Rose and Sean Welleck. *Argument Reconstruction as Supervision for Critical Thinking in LLMs*. [arXiv:2603.17432](https://arxiv.org/abs/2603.17432), 2026. It introduces the GAAR reconstruction engine and the Arguinas dataset.
- Arguinas code and data: <https://github.com/GyoukChu/Arguinas>
- Argdown: <https://argdown.org>
