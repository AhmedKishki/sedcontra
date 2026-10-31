//! The MCP front end: one tool, `videtur_quod`, over stdio.

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ProgressNotificationParam, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use tokio::sync::mpsc;

use crate::run::{Environment, Progress, RunFailure, cross_examine};

const TOOL_DESCRIPTION: &str = "Cross-examine a proposal for invented connections, unstated or unneeded premises, and text its argument does not use. An advocate reconstructs the proposal's argument as a tree of claims and inferences, and an adversary critiques it under explanatory-critique: a critique counts only if it shows a case where an inference's premises hold and its conclusion fails, derives a conclusion without one of its premises, shows that a premise asserts the conclusion, or quotes a misreading. The advocate accepts, adapts, concedes or rejects each critique. Each round ends with the adversary's critique, and the advocate answers it at the start of the next round, so a critique raised in the last round comes back unanswered for you to weigh. The debate ends when the adversary is satisfied or the configured round limit is reached. The report holds a ledger of every critique, the final tree in Argdown, every change the debate forced, the inferences conceded to fail, the critiques left open or unanswered, restatements, uncovered text and stubs to consider pruning, and the full transcript. It describes the proposal and does not judge it. The call can take several minutes.";

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct VideturQuodArgs {
    /// The text to cross-examine, of any length. The advocate and the adversary see nothing else of your work, so state the argument in full.
    pub proposal: String,
    /// Optional: what you need the proposal for, and any evidence you already have. The advocate and the adversary argue from the proposal, this context and their own knowledge.
    pub context: Option<String>,
}

pub struct Server {
    env: Arc<Environment>,
}

#[tool_router]
impl Server {
    pub fn new(env: Environment) -> Self {
        Server { env: Arc::new(env) }
    }

    #[tool(name = "videtur_quod", description = TOOL_DESCRIPTION)]
    async fn videtur_quod(
        &self,
        Parameters(args): Parameters<VideturQuodArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let (tx, mut rx) = mpsc::unbounded_channel::<Progress>();
        let forwarder = ctx.meta.get_progress_token().map(|token| {
            let peer = ctx.peer.clone();
            tokio::spawn(async move {
                while let Some(p) = rx.recv().await {
                    let param = ProgressNotificationParam::new(token.clone(), p.done as f64)
                        .with_total(p.max as f64)
                        .with_message(p.message);
                    let _ = peer.notify_progress(param).await;
                }
            })
        });

        let result = cross_examine(&self.env, args.proposal, args.context, Some(tx), ctx.ct.clone()).await;
        if let Some(forwarder) = forwarder {
            let _ = forwarder.await;
        }

        match result {
            Ok(output) => {
                let mut result = CallToolResult::success(vec![ContentBlock::text(output.markdown)]);
                result.structured_content = Some(output.json);
                Ok(result)
            }
            Err(RunFailure::Failed { reason, record_dir, transcript }) => {
                let record = record_dir.map(|d| format!(" Run record: {}.", d.display())).unwrap_or_default();
                let text = format!("Cross-examination failed: {reason}.{record}\n\n{transcript}");
                Ok(CallToolResult::error(vec![ContentBlock::text(text.trim_end())]))
            }
            Err(RunFailure::Cancelled { .. }) => Err(ErrorData::internal_error("cancelled", None)),
        }
    }
}

#[tool_handler]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(TOOL_DESCRIPTION)
    }
}

pub async fn serve(env: Environment) -> anyhow::Result<()> {
    let service = Server::new(env).serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
