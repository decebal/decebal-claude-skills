use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::catalog::{listing_cost, rating_key, Host, Skill, Source};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A directory under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> TempDir {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let path = std::env::temp_dir().join(format!(
            "skill-rank-test-{}-{label}-{n}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, text).expect("write fixture");
}

pub fn skill_md(name: &str, description: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n")
}

pub fn skill(name: &str, description: &str, host: Host, source: Source) -> Skill {
    let plugin = (source == Source::Plugin).then(|| "plug".to_owned());
    let display = match &plugin {
        Some(p) => format!("{p}:{name}"),
        None => name.to_owned(),
    };
    Skill {
        name: name.to_owned(),
        description: description.to_owned(),
        dir_name: name.to_owned(),
        path: format!("/skills/{}/{name}/SKILL.md", host.as_str()),
        host,
        source,
        plugin,
        controllable: source == Source::User,
        cost: listing_cost(host, &display, description),
        key: rating_key(name, description),
        tier: source.default_tier().to_owned(),
    }
}
