//! One folder per run: inputs, resolved settings, every turn, the status and the report.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::agents::AgentDef;
use crate::config::Config;
use crate::debate::TurnEvent;

#[derive(Clone, Debug)]
pub struct RunRecord {
    pub id: String,
    pub dir: PathBuf,
}

impl RunRecord {
    pub fn create(
        runs_dir: &Path,
        proposal: &str,
        context: Option<&str>,
        config: &Config,
        agents: &[&AgentDef],
    ) -> anyhow::Result<Self> {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let id = format!("{}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), &suffix[..6]);
        let dir = runs_dir.join(&id);
        fs::create_dir_all(dir.join("turns")).with_context(|| format!("creating {}", dir.display()))?;
        fs::create_dir_all(dir.join("agents"))?;

        fs::write(dir.join("proposal.md"), proposal)?;
        if let Some(context) = context {
            fs::write(dir.join("context.md"), context)?;
        }
        fs::write(dir.join("config.toml"), toml::to_string(config)?)?;
        for agent in agents {
            let header = format!(
                "source: {}\nprovider: {}\nmodel: {}\nmax_tokens: {}\nrequest: {}\n\n",
                agent.source,
                agent.model.provider,
                agent.model.model,
                agent.model.max_tokens,
                serde_json::Value::Object(agent.model.request.clone())
            );
            fs::write(dir.join("agents").join(format!("{}.md", agent.name)), header + &agent.prompt)?;
        }

        let record = RunRecord { id, dir };
        record.set_status("running")?;
        Ok(record)
    }

    pub fn write_turn(&self, ev: &TurnEvent) -> anyhow::Result<()> {
        let stem = format!("{:02}-{}-r{}", ev.index, ev.turn.role(), ev.turn.round());
        let turns = self.dir.join("turns");
        fs::write(turns.join(format!("{stem}.json")), &ev.raw)?;
        fs::write(turns.join(format!("{stem}.parsed.json")), serde_json::to_string_pretty(&ev.turn)?)?;
        Ok(())
    }

    pub fn set_status(&self, status: &str) -> anyhow::Result<()> {
        fs::write(self.dir.join("status"), format!("{status}\n"))?;
        Ok(())
    }

    pub fn write_report(&self, markdown: &str, json: &serde_json::Value) -> anyhow::Result<()> {
        fs::write(self.dir.join("report.md"), markdown)?;
        fs::write(self.dir.join("report.json"), serde_json::to_string_pretty(json)?)?;
        Ok(())
    }
}
