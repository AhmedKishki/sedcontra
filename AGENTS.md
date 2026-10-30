# Agent rules

## Code

- Every change keeps to the Constraints in SPEC.md.
- Keep the core, meaning the data model, the debate loop and the tree checks, free of any one provider, front end, analyzer or renderer. Add a provider behind `ModelProvider`, an analyzer behind `Analyzer`, and a renderer or front end beside the existing ones, so that each part can change without touching the core.
- Put each configurable default and each provider parameter in `config/default.toml`, never in code or prompts. A fixed limit that stays in code is a named constant whose doc comment gives its reason, as `MAX_ROUNDS` does.
- An agent file in `agents/` holds only its prompt, so that the configuration alone decides which model runs it.
- Keep the advocate and the adversary isolated: each sees only the messages the orchestrator sends it, and neither reads the other's state.
- Hold an API key only in `ApiKey`, and pass every provider error through `ApiKey::redact` before it reaches a report, an MCP result, stderr or the run record, because a provider may echo the key back.
- Return errors instead of panicking.
  - Use `thiserror` for an error whose cases a caller tells apart, and `anyhow` with context elsewhere.
  - Use `unwrap` only in tests and test support, and `expect` only on an invariant the code guarantees, with the invariant as its message.
- Name things with SPEC.md's terms, such as claim, inference, place, stub, critique and ledger, so that the code, the prompts and the docs read alike.
- Write a comment only for a reason or an invariant the code cannot show, as a full sentence. Open each module root with a `//!` line saying what the module does.
- Keep `cargo clippy --all-targets` free of warnings.
- Match the layout of the surrounding code. Do not run `cargo fmt` over the tree: the project has no rustfmt configuration, so it would rewrite unrelated lines.
- Add a crate only when no existing dependency covers the need.

## Documentation

- SPEC.md holds the decided design. A change to the behaviour it decides updates SPEC.md in the same commit.
- README.md holds, in this order, the logo, the story of the name, the principle, the architecture with its diagram, the installation guide and the references. Put any other fact in SPEC.md, `config/default.toml` or the code.
- State each decision as it stands now. Dates, history and the reasons for a change go in the commit message, where git keeps them.
- Give every sentence a connection to its neighbours, such as a reason, a condition, a consequence or a contrast, and cut a sentence that connects to nothing. Describe without selling words.

## Tests

- Do not use test-driven development, even when a skill prescribes it. Code that compiles counts as done.
- Write tests only where compiling code can still produce a wrong report without a visible error:
  - the tree checks, especially loop detection and the word-for-word quote check;
  - the analyzers, on hand-built trees;
  - the ending of the debate: the early stop at the first satisfied adversary turn and the split into open and unanswered critiques;
  - the key handling: owner-only storage, the refusal of a key file others can read, and redaction.
- Put each area's tests in one file under `tests/`. A test that needs a model drives it with `ScriptedProvider`, never the network or a real key.
- Run `cargo test` before committing a change that touches a tested area.
- Check behaviour against a live model with `sedcontra eval`, not with a test.

## Repository

- Commit one logical change at a time, together with the SPEC.md, README.md and configuration updates it needs.
- Write the subject as `type(scope): summary`, with the type `feat`, `fix`, `refactor`, `docs` or `chore` and an optional scope. Give what changed and why in the body.
- Keep each evaluation's results, with the commit and the settings they ran under, in a folder of their own under `eval/`.
- Never force-add a file that `.gitignore` excludes. Give a new ignore entry a comment saying why.
- Never run `sedcontra key set`, read `~/.config/sedcontra/keys/` or ask for an API key. The owner stores keys in their own terminal, so that no key enters an agent's context.
