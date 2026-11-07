//! The repo side: where skills and hook templates live, and how they are read.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Skill {
    pub name: String,
    pub dir: PathBuf,
}

pub struct Repo {
    pub root: PathBuf,
}

impl Repo {
    /// The explicit root, or the nearest ancestor of the working directory that
    /// holds both `skills/` and `configs/`.
    pub fn locate(explicit: Option<&Path>) -> Result<Self, String> {
        if let Some(root) = explicit {
            let repo = Self {
                root: root.to_path_buf(),
            };
            if repo.root.join("skills").is_dir() && repo.root.join("configs").is_dir() {
                return Ok(repo);
            }
            return Err(format!(
                "{} has no skills/ and configs/ directories",
                root.display()
            ));
        }
        let cwd = std::env::current_dir()
            .map_err(|e| format!("cannot read the working directory: {e}"))?;
        for dir in cwd.ancestors() {
            if dir.join("skills").is_dir() && dir.join("configs").is_dir() {
                return Ok(Self {
                    root: dir.to_path_buf(),
                });
            }
        }
        Err("no repo above the working directory; pass --repo <dir>".into())
    }

    /// Every `skills/<name>/` that carries a SKILL.md, sorted by name.
    pub fn skills(&self) -> Result<Vec<Skill>, String> {
        let dir = self.root.join("skills");
        let entries =
            fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && path.is_dir() && path.join("SKILL.md").is_file() {
                out.push(Skill { name, dir: path });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn template(&self, name: &str) -> Result<Value, String> {
        read_json(&self.root.join("configs").join(name))
    }
}

pub fn read_json(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))
}

pub fn sha256(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
