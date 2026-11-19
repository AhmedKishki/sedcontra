//! Build script: reads the checked-out commit from git's own files, so that the build runs no program, and passes it to the crate as SEDCONTRA_COMMIT.

use std::env;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

/// The value when no commit can be resolved, as in a source archive without a `.git`.
const UNKNOWN: &str = "unknown";

fn main() {
    let mut watched = Vec::new();
    let commit = resolve(&mut watched).unwrap_or_else(|| UNKNOWN.to_string());
    for path in &watched {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rustc-env=SEDCONTRA_COMMIT={commit}");
}

/// The hash of the commit that HEAD points to, or `None` when any step of the lookup fails.
fn resolve(watched: &mut Vec<PathBuf>) -> Option<String> {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    let git = manifest.join(".git");
    let gitdir = if git.is_file() {
        let text = read_file(&git, watched).ok()??;
        manifest.join(text.trim().strip_prefix("gitdir: ")?)
    } else if git.is_dir() {
        git
    } else {
        return None;
    };
    let common = match read_file(&gitdir.join("commondir"), watched).ok()? {
        Some(text) => gitdir.join(text.trim()),
        None => gitdir.clone(),
    };

    let head = read_file(&gitdir.join("HEAD"), watched).ok()??;
    let head = head.trim();
    let hash = match head.strip_prefix("ref: ") {
        Some(name) => ref_hash(name, &gitdir, &common, watched)?,
        None => head.to_string(),
    };
    (matches!(hash.len(), 40 | 64) && hash.chars().all(|c| c.is_ascii_hexdigit())).then_some(hash)
}

/// The hash a ref names: the first loose ref file that exists, else the entry in `packed-refs`.
fn ref_hash(name: &str, gitdir: &Path, common: &Path, watched: &mut Vec<PathBuf>) -> Option<String> {
    let mut loose = vec![gitdir.join(name)];
    if common != gitdir {
        loose.push(common.join(name));
    }
    for path in &loose {
        if let Some(text) = read_file(path, watched).ok()? {
            return Some(text.trim().to_string());
        }
    }
    // No loose ref file exists, so the probed paths stay watched while missing: the next commit writes the loose ref file, and Cargo reruns the script on every build while a watched path is missing.
    watched.extend(loose);
    let packed = common.join("packed-refs");
    watched.push(packed.clone());
    let packed = read_text(&packed).ok()??;
    packed.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let hash = fields.next()?;
        (fields.next() == Some(name)).then(|| hash.to_string())
    })
}

/// Reads a text file and keeps its path in `watched` when it exists, so that the script reruns when the file changes.
fn read_file(path: &Path, watched: &mut Vec<PathBuf>) -> io::Result<Option<String>> {
    let text = read_text(path)?;
    if text.is_some() {
        watched.push(path.to_path_buf());
    }
    Ok(text)
}

/// Reads a text file. A missing file is `Ok(None)`.
fn read_text(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
