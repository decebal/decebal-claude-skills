use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::apply::{self, Plan};
use crate::catalog::{sha256_hex, Catalog, Host, Source};
use crate::fmt;
use crate::jev::{self, HttpTransport, Redactor};
use crate::rate::{self, Options, RUN_BUDGET};
use crate::{scan, search, usage};

pub const USAGE: &str = "\
usage: skill-rank <command> [options]

  scan   [--days 60]                     inventory Claude and Codex skills and their recent use
  rate   [--dry-run] [--force] [--batch 20] [--model jev-latest]
                                         rate unrated skills with TypeSafe Jev
  apply  --host claude|codex [--budget-chars N | --context-tokens N] [--dry-run]
                                         keep the best-rated skills listed within the budget
  search <words...> [--host claude|codex] [--limit 8]
                                         find a skill, ranked by relevance and rating
  list   [--host claude|codex]           every skill by rating

data: $SKILL_RANK_HOME, else ~/.config/skill-rank (profile.md, redact.txt, catalog.json, jev-log/)
";

const VALUED: [&str; 7] = [
    "days",
    "batch",
    "model",
    "host",
    "budget-chars",
    "context-tokens",
    "limit",
];
const SWITCHES: [&str; 2] = ["dry-run", "force"];
const SECONDS_PER_DAY: u64 = 86_400;
const BACKOFF: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(5)];

pub struct Env {
    pub home: PathBuf,
    pub data_dir: PathBuf,
    pub now: u64,
}

impl Env {
    pub fn from_process() -> Result<Env, String> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .ok_or("HOME is not set")?;
        let data_dir = std::env::var_os("SKILL_RANK_HOME")
            .filter(|h| !h.is_empty())
            .map_or_else(|| home.join(".config/skill-rank"), PathBuf::from);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Ok(Env {
            home,
            data_dir,
            now,
        })
    }

    pub fn catalog_path(&self) -> PathBuf {
        self.data_dir.join("catalog.json")
    }

    fn profile_path(&self) -> PathBuf {
        self.data_dir.join("profile.md")
    }

    fn redact_path(&self) -> PathBuf {
        self.data_dir.join("redact.txt")
    }

    fn log_dir(&self) -> PathBuf {
        self.data_dir.join("jev-log")
    }
}

/// `text` goes to stdout when `code` is 0, to stderr otherwise.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub text: String,
    pub code: u8,
}

fn ok(text: impl Into<String>) -> Outcome {
    Outcome {
        text: text.into(),
        code: 0,
    }
}

#[derive(Debug, Default)]
struct Flags {
    values: BTreeMap<String, String>,
    switches: BTreeSet<String>,
    words: Vec<String>,
}

impl Flags {
    fn parse(args: &[String]) -> Result<Flags, String> {
        let mut flags = Flags::default();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            let Some(flag) = arg.strip_prefix("--") else {
                flags.words.push(arg.clone());
                continue;
            };
            let (name, inline) = match flag.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (flag, None),
            };
            if VALUED.contains(&name) {
                let value = inline
                    .or_else(|| it.next().cloned())
                    .ok_or_else(|| format!("--{name} needs a value"))?;
                flags.values.insert(name.to_owned(), value);
            } else if SWITCHES.contains(&name) && inline.is_none() {
                flags.switches.insert(name.to_owned());
            } else {
                return Err(format!("unknown option --{name}\n{USAGE}"));
            }
        }
        Ok(flags)
    }

    fn switch(&self, name: &str) -> bool {
        self.switches.contains(name)
    }

    fn opt_num(&self, name: &str) -> Result<Option<u64>, String> {
        self.values
            .get(name)
            .map(|v| {
                v.parse::<u64>()
                    .map_err(|_| format!("--{name} takes a whole number, got {v:?}"))
            })
            .transpose()
    }

    fn num(&self, name: &str, default: u64) -> Result<u64, String> {
        Ok(self.opt_num(name)?.unwrap_or(default))
    }

    fn host(&self) -> Result<Option<Host>, String> {
        self.values
            .get("host")
            .map(|h| {
                Host::parse(h).ok_or_else(|| format!("--host takes claude or codex, got {h:?}"))
            })
            .transpose()
    }

    fn no_words(&self, command: &str) -> Result<(), String> {
        match self.words.first() {
            Some(w) => Err(format!("{command} takes no argument {w:?}\n{USAGE}")),
            None => Ok(()),
        }
    }
}

pub fn run(args: &[String], env: &Env) -> Result<Outcome, String> {
    let Some((command, rest)) = args.split_first() else {
        return Ok(Outcome {
            text: USAGE.to_owned(),
            code: 1,
        });
    };
    let flags = Flags::parse(rest)?;
    match command.as_str() {
        "scan" => cmd_scan(env, &flags),
        "rate" => cmd_rate(env, &flags),
        "apply" => cmd_apply(env, &flags),
        "search" => cmd_search(env, &flags),
        "list" => cmd_list(env, &flags),
        "help" | "-h" | "--help" => Ok(ok(USAGE)),
        other => Err(format!("unknown command {other:?}\n{USAGE}")),
    }
}

/// The catalog, with ratings made against another profile.md reading as missing.
fn load_catalog(env: &Env) -> Result<Catalog, String> {
    let mut catalog = Catalog::load(&env.catalog_path())?
        .ok_or_else(|| "no catalog yet: run `skill-rank scan` first".to_owned())?;
    catalog.current_profile = current_profile(env)?;
    Ok(catalog)
}

fn current_profile(env: &Env) -> Result<Option<String>, String> {
    Ok(read_optional(&env.profile_path())?
        .filter(|p| !p.trim().is_empty())
        .map(|p| sha256_hex(p.as_bytes())))
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Writes `text` over `path` and backs up what was there.
///
/// A symlink is written through to its target. `<file>.skill-rank.orig` keeps
/// the file as it was before skill-rank first changed it and is never
/// overwritten; `<file>.skill-rank.bak` holds the version just replaced. Both
/// backups take the original's permissions, since `~/.codex/config.toml` can
/// hold tokens. The new text goes to a temp file renamed into place, so a
/// reader never sees half a file, and the write is refused when the file
/// changed after it was read.
pub fn write_with_backup(
    path: &Path,
    original: Option<&str>,
    text: &str,
) -> Result<String, String> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let changed = |target: &Path| -> Result<bool, String> {
        Ok(read_optional(target)?.as_deref() != original)
    };
    let refuse = || {
        format!(
            "{} changed while skill-rank was planning; nothing written, run apply again",
            target.display()
        )
    };
    if changed(&target)? {
        return Err(refuse());
    }
    let mode = fs::metadata(&target).ok().map(|m| m.permissions());
    let mut note = String::new();
    if let Some(original) = original {
        let orig = sibling(&target, ".skill-rank.orig");
        if !orig.exists() {
            write_file(&orig, original, mode.as_ref())?;
        }
        let bak = sibling(&target, ".skill-rank.bak");
        write_file(&bak, original, mode.as_ref())?;
        note = format!(
            " (previous version in {}; the version before skill-rank in {})",
            bak.display(),
            orig.display()
        );
    } else if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let tmp = sibling(&target, ".skill-rank.tmp");
    write_file(&tmp, text, mode.as_ref())?;
    if changed(&target)? {
        let _ = fs::remove_file(&tmp);
        return Err(refuse());
    }
    fs::rename(&tmp, &target).map_err(|e| format!("write {}: {e}", target.display()))?;
    Ok(format!("wrote {}{note}\n", target.display()))
}

/// Writes `text` to `path`, created with the given permissions (owner-only
/// when none are known) before any byte lands in it.
fn write_file(path: &Path, text: &str, mode: Option<&fs::Permissions>) -> Result<(), String> {
    use std::io::Write as _;
    let fail = |e: std::io::Error| format!("write {}: {e}", path.display());
    let _ = fs::remove_file(path);
    let mut open = fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        open.mode(mode.map_or(0o600, |m| m.mode() & 0o777));
    }
    let mut file = open.open(path).map_err(fail)?;
    file.write_all(text.as_bytes()).map_err(fail)?;
    file.sync_all().map_err(fail)?;
    if let Some(mode) = mode {
        fs::set_permissions(path, mode.clone()).map_err(fail)?;
    }
    Ok(())
}

fn cmd_scan(env: &Env, flags: &Flags) -> Result<Outcome, String> {
    flags.no_words("scan")?;
    let days = flags.num("days", 60)?;
    let since = UNIX_EPOCH
        + Duration::from_secs(env.now.saturating_sub(days.saturating_mul(SECONDS_PER_DAY)));
    let prev = Catalog::load(&env.catalog_path())?.unwrap_or_default();
    let prev_tiers: BTreeMap<(Host, &str), &str> = prev
        .skills
        .iter()
        .map(|s| ((s.host, s.path.as_str()), s.tier.as_str()))
        .collect();

    let mut skills = scan::discover(&env.home);
    for s in &mut skills {
        if s.source == Source::User {
            if let Some(tier) = prev_tiers.get(&(s.host, s.path.as_str())) {
                (*tier).clone_into(&mut s.tier);
            }
        }
    }
    let keys: BTreeSet<&str> = skills.iter().map(|s| s.key.as_str()).collect();
    let ratings = prev
        .ratings
        .iter()
        .filter(|(k, _)| keys.contains(k.as_str()))
        .map(|(k, r)| (k.clone(), r.clone()))
        .collect();
    // Each host gets half the limit: Claude transcripts alone can use all of it.
    let half = || usage::ScanBudget::new(usage::SCAN_TIME / 2, usage::SCAN_BYTES / 2);
    let (mut claude_budget, mut codex_budget) = (half(), half());
    let mut usage_by_host = BTreeMap::new();
    usage_by_host.insert(
        Host::Claude,
        usage::scan_claude(&env.home, since, &mut claude_budget),
    );
    usage_by_host.insert(
        Host::Codex,
        usage::scan_codex(&env.home, since, &mut codex_budget),
    );
    let truncated = claude_budget.truncated || codex_budget.truncated;
    let catalog = Catalog {
        scanned_at: env.now,
        days,
        skills,
        usage: usage_by_host,
        ratings,
        managed: prev.managed.clone(),
        current_profile: current_profile(env)?,
        usage_truncated: truncated,
    };
    catalog.save(&env.catalog_path())?;
    Ok(ok(scan_summary(&catalog, &env.catalog_path())))
}

pub fn scan_summary(catalog: &Catalog, path: &Path) -> String {
    let mut out = String::new();
    for host in Host::ALL {
        let on_host: Vec<_> = catalog.skills.iter().filter(|s| s.host == host).collect();
        let by_source: Vec<String> = Source::ALL
            .iter()
            .filter_map(|src| {
                let n = on_host.iter().filter(|s| s.source == *src).count();
                (n > 0).then(|| format!("{} {n}", src.as_str()))
            })
            .collect();
        let chars: u64 = on_host.iter().map(|s| s.cost).sum();
        let _ = writeln!(
            out,
            "{}: {} skills ({}); {} listing chars with every skill on",
            host.as_str(),
            on_host.len(),
            if by_source.is_empty() {
                "none".to_owned()
            } else {
                by_source.join(", ")
            },
            fmt::thousands(chars)
        );
    }
    let used: Vec<String> = Host::ALL
        .iter()
        .map(|h| {
            let m = catalog.usage.get(h);
            let names = m.map_or(0, BTreeMap::len);
            let calls: u64 = m.map_or(0, |m| m.values().map(|u| u.count).sum());
            format!("{} {names} skills, {calls} uses", h.as_str())
        })
        .collect();
    let _ = writeln!(
        out,
        "used in the last {} days: {}",
        catalog.days,
        used.join("; ")
    );
    if catalog.usage_truncated {
        let _ = writeln!(
            out,
            "the usage scan hit its limit ({}s or {} GiB per host); the oldest transcripts were not read",
            usage::SCAN_TIME.as_secs() / 2,
            usage::SCAN_BYTES >> 31
        );
    }
    let keys: BTreeSet<&str> = catalog.skills.iter().map(|s| s.key.as_str()).collect();
    let rated = catalog
        .skills
        .iter()
        .filter(|s| catalog.rating_of(s).is_some())
        .map(|s| s.key.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let stale = catalog.stale_ratings();
    let _ = writeln!(
        out,
        "rated {rated} of {} distinct skills{}",
        keys.len(),
        if stale > 0 {
            format!("; {stale} more were rated against an older profile.md and count as unrated")
        } else {
            String::new()
        }
    );
    let _ = writeln!(out, "wrote {}", path.display());
    out
}

fn cmd_rate(env: &Env, flags: &Flags) -> Result<Outcome, String> {
    flags.no_words("rate")?;
    let profile = read_optional(&env.profile_path())?.unwrap_or_default();
    if profile.trim().is_empty() {
        return Ok(Outcome {
            text: format!(
                "no profile: write {} first. A few lines on the work you do (stacks, kinds of tasks, \
                 what you ship) is enough; it is sent to Jev with every rating request.\n",
                env.profile_path().display()
            ),
            code: 2,
        });
    }
    let redact = Redactor::from_text(&read_optional(&env.redact_path())?.unwrap_or_default())?;
    let mut catalog = Catalog::load(&env.catalog_path())?
        .ok_or_else(|| "no catalog yet: run `skill-rank scan` first".to_owned())?;
    let opts = Options {
        batch: usize::try_from(flags.num("batch", 20)?.max(1)).unwrap_or(usize::MAX),
        model: flags
            .values
            .get("model")
            .cloned()
            .unwrap_or_else(|| jev::DEFAULT_MODEL.to_owned()),
        force: flags.switch("force"),
        budget: RUN_BUDGET,
        backoff: BACKOFF,
        now_unix: env.now,
    };
    let todo = rate::pending(&catalog, &sha256_hex(profile.as_bytes()), opts.force).len();
    if todo == 0 {
        return Ok(ok(
            "nothing to rate: every skill has a rating for this profile (--force rates again)\n",
        ));
    }
    let batches = todo.div_ceil(opts.batch);

    if flags.switch("dry-run") {
        let request =
            rate::first_request(&catalog, &profile, &redact, &opts).unwrap_or(Value::Null);
        let pretty = serde_json::to_string_pretty(&request).map_err(|e| e.to_string())?;
        return Ok(ok(format!(
            "{todo} skills to rate in {batches} requests; the first request follows. Nothing was sent.\n{pretty}\n"
        )));
    }

    let key = jev::api_key(&env.home).ok_or(
        "no TypeSafe key: set TYPESAFE_API_KEY or write the key to ~/.config/typesafe/key",
    )?;
    let transport = HttpTransport::new(key);
    let path = env.catalog_path();
    let mut save = |c: &Catalog| c.save(&path);
    let report = rate::run(
        &mut catalog,
        &profile,
        &redact,
        &opts,
        &transport,
        Some(&env.log_dir()),
        &mut save,
    );
    let mut text = format!(
        "rated {} of {} skills in {} of {batches} requests; requests and answers in {}\n",
        report.rated,
        report.pending,
        report.batches,
        env.log_dir().display()
    );
    for f in &report.failures {
        let _ = writeln!(text, "failed: {f}");
    }
    if let Some(stop) = &report.stopped {
        let _ = writeln!(text, "stopped: {stop}");
    }
    let code = u8::from(report.stopped.is_some() || !report.failures.is_empty());
    Ok(Outcome { text, code })
}

fn cmd_apply(env: &Env, flags: &Flags) -> Result<Outcome, String> {
    flags.no_words("apply")?;
    let host = flags
        .host()?
        .ok_or("apply needs --host claude or --host codex")?;
    if flags.values.contains_key("budget-chars") && flags.values.contains_key("context-tokens") {
        return Err("give --budget-chars or --context-tokens, not both".to_owned());
    }
    let explicit = flags.opt_num("budget-chars")?;
    let configured = if host == Host::Codex
        && explicit.is_none()
        && !flags.values.contains_key("context-tokens")
    {
        read_optional(&env.home.join(".codex/config.toml"))?
            .as_deref()
            .and_then(apply::codex_max_context_tokens)
            .map(|tokens| tokens * apply::CHARS_PER_TOKEN)
    } else {
        None
    };
    let budget = apply::budget_chars(
        host,
        explicit.or(configured),
        flags.num("context-tokens", apply::DEFAULT_CONTEXT_TOKENS)?,
    );
    let mut catalog = load_catalog(env)?;
    let dry = flags.switch("dry-run");
    match host {
        Host::Claude => apply_claude(env, &mut catalog, budget, dry),
        Host::Codex => apply_codex(env, &mut catalog, budget, dry),
    }
}

fn record_tiers(catalog: &mut Catalog, plan: &Plan) {
    for s in catalog.skills.iter_mut().filter(|s| s.host == plan.host) {
        let tier = if s.controllable {
            plan.tier_of(&s.name).map_or("on", apply::Tier::as_str)
        } else {
            s.source.default_tier()
        };
        tier.clone_into(&mut s.tier);
    }
}

fn apply_claude(
    env: &Env,
    catalog: &mut Catalog,
    budget: u64,
    dry: bool,
) -> Result<Outcome, String> {
    let path = env.home.join(".claude/settings.json");
    let original = read_optional(&path)?;
    let settings: Value = match &original {
        Some(t) => serde_json::from_str(t)
            .map_err(|e| format!("refusing to write: {} does not parse: {e}", path.display()))?,
        None => json!({}),
    };
    let managed = catalog.managed_for(Host::Claude).to_vec();
    let user = apply::claude_user_overrides(&settings, &managed);
    let plan = apply::plan(catalog, Host::Claude, budget, &user);
    let mut text = apply::render_plan(&plan);
    if dry {
        text.push_str("\ndry run: nothing written\n");
        return Ok(ok(text));
    }
    let merged = apply::merge_claude(settings, &plan, &managed)?;
    text.push('\n');
    if merged.changed {
        let body = apply::settings_text(&merged.settings)?;
        text.push_str(&write_with_backup(&path, original.as_deref(), &body)?);
    } else {
        text.push_str("settings.json already matches the plan; nothing written\n");
    }
    catalog.managed.insert(Host::Claude, merged.managed);
    record_tiers(catalog, &plan);
    catalog.save(&env.catalog_path())?;
    Ok(ok(text))
}

fn apply_codex(
    env: &Env,
    catalog: &mut Catalog,
    budget: u64,
    dry: bool,
) -> Result<Outcome, String> {
    let path = env.home.join(".codex/config.toml");
    let original = read_optional(&path)?;
    let config = original.clone().unwrap_or_default();
    let plan = apply::plan(catalog, Host::Codex, budget, &BTreeMap::new());
    let mut text = apply::render_plan(&plan);
    let disabled = plan.demoted();
    if dry {
        let foreign = apply::foreign_skill_config(&config);
        if !foreign.is_empty() {
            let _ = writeln!(
                text,
                "\napply would refuse: config.toml has [[skills.config]] outside the skill-rank block:\n{}",
                foreign.join("\n")
            );
        }
        text.push_str("\ndry run: nothing written\n");
        return Ok(ok(text));
    }
    let updated = match apply::merge_codex(&config, &disabled) {
        Ok(t) => t,
        Err(e) => {
            return Ok(Outcome {
                text: format!("{text}\n{e}\n"),
                code: 1,
            })
        }
    };
    text.push('\n');
    if updated == config {
        text.push_str("config.toml already matches the plan; nothing written\n");
    } else {
        text.push_str(&write_with_backup(&path, original.as_deref(), &updated)?);
    }
    catalog.managed.insert(Host::Codex, disabled);
    record_tiers(catalog, &plan);
    catalog.save(&env.catalog_path())?;
    Ok(ok(text))
}

fn one_line(s: &str, max: usize) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    jev::truncate_chars(&flat, max)
}

fn cmd_search(env: &Env, flags: &Flags) -> Result<Outcome, String> {
    if flags.words.is_empty() {
        return Err(format!("search needs words\n{USAGE}"));
    }
    let catalog = load_catalog(env)?;
    let limit = usize::try_from(flags.num("limit", 8)?).unwrap_or(usize::MAX);
    let hits = search::search(&catalog, &flags.words, flags.host()?, limit);
    if hits.is_empty() {
        return Ok(ok(format!(
            "no skill matches {:?}\n",
            flags.words.join(" ")
        )));
    }
    let mut out = String::new();
    for h in hits {
        let s = &catalog.skills[h.index];
        let _ = writeln!(
            out,
            "{:.2}  {}  {}  {}  {}  {}\n    {}",
            h.score,
            fmt::rating(catalog.rating_of(s)),
            s.tier,
            s.host.as_str(),
            s.display_name(),
            s.path,
            one_line(&s.description, 160)
        );
    }
    Ok(ok(out))
}

fn cmd_list(env: &Env, flags: &Flags) -> Result<Outcome, String> {
    flags.no_words("list")?;
    let catalog = load_catalog(env)?;
    let host = flags.host()?;
    let mut rows: Vec<_> = catalog
        .skills
        .iter()
        .filter(|s| host.is_none_or(|h| s.host == h))
        .collect();
    rows.sort_by(|a, b| {
        let (ra, rb) = (catalog.rating_of(a), catalog.rating_of(b));
        rb.is_some()
            .cmp(&ra.is_some())
            .then_with(|| rb.unwrap_or(0.0).total_cmp(&ra.unwrap_or(0.0)))
            .then_with(|| a.display_name().cmp(&b.display_name()))
            .then_with(|| a.host.cmp(&b.host))
    });
    let mut out = format!(
        "{:>6}  {:<9}  {:<6}  {:>9}  name\n",
        "rating", "tier", "host", "use c/x"
    );
    for s in rows {
        let (claude, codex) = catalog.uses(s);
        let _ = writeln!(
            out,
            "{:>6}  {:<9}  {:<6}  {:>9}  {}",
            fmt::rating(catalog.rating_of(s)),
            s.tier,
            s.host.as_str(),
            format!("{claude}/{codex}"),
            s.display_name()
        );
    }
    Ok(ok(out))
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
