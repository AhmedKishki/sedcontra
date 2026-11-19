//! Reads a case from its folder: a proposal and the defects planted at quoted places in it.

use std::path::Path;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::check::{QuoteIndex, Span, missing_quote_message};

/// One planted defect as `defects.toml` states it.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Planted {
    pub kind: String,
    pub places: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// The `defects.toml` file: zero or more `[[defect]]` tables.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefectsFile {
    #[serde(default)]
    defect: Vec<Planted>,
}

/// A planted defect with its id and the spans of its places in the proposal.
#[derive(Clone, Debug)]
pub struct Defect {
    pub id: String,
    pub kind: String,
    pub places: Vec<String>,
    pub note: Option<String>,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug)]
pub struct Case {
    pub name: String,
    pub proposal: String,
    pub context: Option<String>,
    pub defects: Vec<Defect>,
}

impl Case {
    /// Gives the planted defects the ids D1, D2, … in the order given and finds their places in
    /// the proposal. A defect that names no kind or quotes no place, and a place that does not
    /// occur, are all returned together, so that one run of the loader shows every failure.
    pub fn new(name: String, proposal: String, context: Option<String>, planted: Vec<Planted>) -> Result<Case, Vec<String>> {
        let index = QuoteIndex::new(&proposal);
        let mut failures = Vec::new();
        let mut defects = Vec::new();
        for (n, planted) in planted.into_iter().enumerate() {
            let id = format!("D{}", n + 1);
            if planted.kind.trim().is_empty() {
                failures.push(format!("defect {id} names no kind"));
            }
            if planted.places.is_empty() {
                failures.push(format!("defect {id} quotes no place"));
            }
            let mut spans = Vec::new();
            for place in &planted.places {
                let found = index.find_all(place);
                if found.is_empty() {
                    failures.push(missing_quote_message(&format!("defect {id}"), place));
                }
                spans.extend(found);
            }
            defects.push(Defect { id, kind: planted.kind, places: planted.places, note: planted.note, spans });
        }
        if failures.is_empty() {
            Ok(Case { name, proposal, context, defects })
        } else {
            Err(failures)
        }
    }
}

/// Loads every case under `dir`: an immediate subfolder with a `proposal.md`, named by its folder
/// and ordered by that name. A case may add a `context.md`, and it must have a `defects.toml`,
/// which a control case leaves empty because it plants nothing.
pub fn load_cases(dir: &Path) -> anyhow::Result<Vec<Case>> {
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry.with_context(|| format!("reading {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() && path.join("proposal.md").is_file() {
            folders.push((entry.file_name().to_string_lossy().into_owned(), path));
        }
    }
    folders.sort();

    let mut cases = Vec::new();
    for (name, folder) in folders {
        cases.push(load_case(&folder, name)?);
    }
    if cases.is_empty() {
        bail!("no case under {}: a case is a folder with proposal.md and defects.toml", dir.display());
    }
    Ok(cases)
}

fn load_case(folder: &Path, name: String) -> anyhow::Result<Case> {
    let proposal_path = folder.join("proposal.md");
    let proposal = std::fs::read_to_string(&proposal_path).with_context(|| format!("reading {}", proposal_path.display()))?;
    let context_path = folder.join("context.md");
    let context = if context_path.exists() {
        Some(std::fs::read_to_string(&context_path).with_context(|| format!("reading {}", context_path.display()))?)
    } else {
        None
    };
    let defects_path = folder.join("defects.toml");
    if !defects_path.exists() {
        bail!("{name}: a case states its defects in defects.toml, and a control case states none with an empty file");
    }
    let text = std::fs::read_to_string(&defects_path).with_context(|| format!("reading {}", defects_path.display()))?;
    let file: DefectsFile = toml::from_str(&text).with_context(|| format!("{name}: {} does not parse", defects_path.display()))?;
    Case::new(name.clone(), proposal, context, file.defect).map_err(|failures| anyhow::anyhow!("{name}: {}", failures.join("; ")))
}
