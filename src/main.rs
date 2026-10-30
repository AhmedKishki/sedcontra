use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use tokio_util::sync::CancellationToken;

use cross_examine::config::Config;
use cross_examine::run::{Environment, RunFailure, cross_examine};
use cross_examine::secret;

/// Adversarial cross-examination of a proposal: an advocate reconstructs its argument, an adversary critiques it.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Configuration file; defaults to ./cross-examine.toml, then the user config directory.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve the cross_examine tool over MCP on stdio.
    Serve,
    /// Cross-examine one proposal and print the report.
    Run {
        /// The proposal file, or - to read stdin.
        proposal: PathBuf,
        /// A file with context: what the proposal is for and any evidence.
        #[arg(long)]
        context: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = Format::Md)]
        format: Format,
        /// Write the report here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Manage provider API keys.
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// Score the tool against a local copy of the Arguinas test split.
    Eval {
        /// The dataset file, e.g. 260331-Arguinas-Test-v2.json.
        dataset: PathBuf,
        /// Score only the first N records.
        #[arg(long)]
        limit: Option<usize>,
        /// Records cross-examined at once.
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
        /// The JSON-lines file for per-record scores; defaults to the runs directory.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum KeyAction {
    /// Store a provider's API key in a file only you can read. Run it in your own terminal:
    /// the key is typed without echo, or read from stdin when piped.
    Set {
        /// The provider name from the configuration, e.g. deepseek.
        provider: String,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum Format {
    Md,
    Json,
    Argdown,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve => {
            let result = match Environment::load(cli.config.as_deref()) {
                Ok(env) => cross_examine::mcp::serve(env).await,
                Err(e) => Err(e),
            };
            match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("cross-examine: {e:#}");
                    ExitCode::from(1)
                }
            }
        }
        Cmd::Run { proposal, context, format, out } => {
            run(cli.config.as_deref(), &proposal, context.as_deref(), format, out.as_deref()).await
        }
        Cmd::Eval { dataset, limit, concurrency, out } => {
            eval(cli.config.as_deref(), &dataset, limit, concurrency, out).await
        }
        Cmd::Key { action: KeyAction::Set { provider } } => set_key(cli.config.as_deref(), &provider),
    }
}

fn set_key(config: Option<&Path>, provider: &str) -> ExitCode {
    let stored = (|| -> anyhow::Result<PathBuf> {
        let config = Config::load(config)?;
        let cfg = config
            .providers
            .get(provider)
            .ok_or_else(|| anyhow::anyhow!("provider {provider} is not in the configuration"))?;
        let path = secret::key_path(provider, cfg)?;
        let key = secret::read_key_from_user(provider)?;
        secret::store_key(&path, &key)?;
        Ok(path)
    })();
    match stored {
        Ok(path) => {
            eprintln!("saved the key for {provider} to {}, readable only by you", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cross-examine: {e:#}");
            ExitCode::from(1)
        }
    }
}

async fn eval(config: Option<&Path>, dataset: &Path, limit: Option<usize>, concurrency: usize, out: Option<PathBuf>) -> ExitCode {
    let env = match Environment::load(config) {
        Ok(env) => env,
        Err(e) => {
            eprintln!("cross-examine: {e:#}");
            return ExitCode::from(1);
        }
    };
    let out = out.unwrap_or_else(|| {
        env.config.runs_dir().join(format!("eval-{}.jsonl", chrono::Local::now().format("%Y%m%d-%H%M%S")))
    });
    let cancel = CancellationToken::new();
    let on_interrupt = cancel.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            on_interrupt.cancel();
        }
        // A second Ctrl-C stops waiting for the cancelled work to wind down.
        if tokio::signal::ctrl_c().await.is_ok() {
            std::process::exit(130);
        }
    });

    match cross_examine::eval::run_eval(env, dataset, limit, concurrency, &out, cancel).await {
        Ok(scores) => {
            print!("{}", cross_examine::eval::summarize(&scores));
            eprintln!("per-record scores: {}", out.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cross-examine: {e:#}");
            ExitCode::from(1)
        }
    }
}

async fn run(config: Option<&Path>, proposal: &Path, context: Option<&Path>, format: Format, out: Option<&Path>) -> ExitCode {
    let fail = |message: String| {
        eprintln!("cross-examine: {message}");
        ExitCode::from(1)
    };
    let env = match Environment::load(config) {
        Ok(env) => env,
        Err(e) => return fail(format!("{e:#}")),
    };
    let proposal = match read_input(proposal) {
        Ok(text) => text,
        Err(e) => return fail(format!("reading the proposal: {e}")),
    };
    let context = match context.map(std::fs::read_to_string).transpose() {
        Ok(text) => text,
        Err(e) => return fail(format!("reading the context: {e}")),
    };

    let cancel = CancellationToken::new();
    let on_interrupt = cancel.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            on_interrupt.cancel();
        }
        // A second Ctrl-C stops waiting for the cancelled work to wind down.
        if tokio::signal::ctrl_c().await.is_ok() {
            std::process::exit(130);
        }
    });

    match cross_examine(&env, proposal, context, None, cancel).await {
        Ok(output) => {
            let text = match format {
                Format::Md => output.markdown,
                Format::Json => serde_json::to_string_pretty(&output.json).unwrap_or_default(),
                Format::Argdown => output.argdown,
            };
            match out {
                Some(path) => match std::fs::write(path, text) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => fail(format!("writing {}: {e}", path.display())),
                },
                None => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
            }
        }
        Err(RunFailure::Failed { reason, record_dir, .. }) => {
            eprintln!("cross-examination failed: {reason}");
            if let Some(dir) = record_dir {
                eprintln!("run record: {}", dir.display());
            }
            ExitCode::from(1)
        }
        Err(RunFailure::Cancelled { record_dir }) => {
            eprintln!("cross-examination cancelled; run record: {}", record_dir.display());
            ExitCode::from(130)
        }
    }
}

fn read_input(path: &Path) -> std::io::Result<String> {
    if path.as_os_str() == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        Ok(text)
    } else {
        std::fs::read_to_string(path)
    }
}
