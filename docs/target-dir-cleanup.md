# Cleaning up Cargo build directories

A machine that builds Rust for several projects and agent worktrees fills its
disk with `target/` directories nobody is using. Measured on one laptop on
2026-09-28: 38 Cargo build directories, 152 GB by `du`, 9.3 GiB free. Deleting
seven idle ones by hand took free space to 122 GiB.

Doing that by hand is where the damage happens. Deleting a build directory that
a dev server writes into, or that a launchd service runs its binary from, breaks
someone's work silently and late. So the job is automated by
[`target-gc`](https://github.com/decebal/target-gc) (on crates.io), which runs
hourly under the platform scheduler and acts only when the disk needs the space.
It ships a Claude Code skill and plugin, so an agent frees space through it
instead of `rm -rf target/`.

## What it does

| | |
|---|---|
| **Finds** | Every directory holding the `CACHEDIR.TAG` Cargo writes, under the configured roots. Names are never evidence: a directory called `target` without Cargo's tag is somebody's source |
| **Holds** | A directory whose Cargo profile lock is held (`<target>/[<triple>/]<profile>/.cargo-lock`, `.cargo-build-lock`, `.cargo-artifact-lock`), whose project is some process's cwd, whose files include a running executable, or whose path appears in a launchd plist, a systemd unit, or a listed config file |
| **Evicts** | Directories idle past `max_idle_days` (default 30) always; under pressure (free below `keep_free`, default 10%), the least recently built until free space reaches `target_free` (default 20%). Nothing built in the last `min_idle_days` (default 2) |
| **Measures** | Free space with `df` before and after every eviction. Copy-on-write clones share blocks, so allocated size overstates what a delete frees |
| **Removes safely** | Re-verifies, takes every profile lock, renames the directory aside, releases the locks, deletes. A Cargo that starts meanwhile waits, then builds cold, never into a half-deleted directory. An interrupted delete is finished by the next run |
| **Reports** | `~/.local/state/target-gc/last-run.json`; the session hook prints one line when the disk is under the floor, the last run failed, or no run has happened in three hours, and nothing otherwise |

## Install

```sh
cargo install target-gc        # or: cargo binstall target-gc
target-gc scan                 # what it sees, and why each directory is held
target-gc run --dry-run        # what it would do today
target-gc install              # hourly schedule (launchd / systemd user timer)
```

`install` refuses to schedule a binary that lives inside a Cargo build
directory, since it could evict itself. The schedule runs once at install
(`RunAtLoad`), so the first expiry pass happens immediately; read
`~/.local/state/target-gc/last-run.json` afterwards to see what it did. Settings live in
`~/.config/target-gc/config.toml`; the crate's
[`config.example.toml`](https://github.com/decebal/target-gc/blob/main/config.example.toml)
is the defaults.

For Claude Code, the plugin brings the skill and the session hook:

```
/plugin marketplace add decebal/target-gc
/plugin install target-gc@target-gc
```

Without the plugin, `target-gc skill install` writes the skill to
`~/.claude/skills/target-gc/`, and the session hook goes in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "$HOME/.cargo/bin/target-gc hook", "timeout": 10 }] }
    ]
  }
}
```

The hook only reads the record the scheduled run leaves and one `df`: 20 ms
measured. It never runs an eviction itself, because a hook-spawned child
outlives the session that started it
([rules/process-ownership.md](../rules/process-ownership.md)).

## Why it was built rather than adopted

Surveyed and checked against source on 2026-09-28:

| Tool | Finding |
|---|---|
| [cargo-sweep](https://github.com/holmgr/cargo-sweep) | Prunes artifacts inside one build directory by age or toolchain. No liveness check. Its own README says it lacks a maintainer |
| [cargo-clean-all](https://github.com/dnlmlr/cargo-clean-all) | Whole-directory deletion by last-compile age and size. Activity is the directory's mtime; no lock or process check; ignores `CARGO_TARGET_DIR` (its issue #27) |
| [kondo](https://github.com/tbillington/kondo), cargo-wipe, cargo-clean-recursive, cargo-cleaner | Artifact cleaners across ecosystems. No liveness check, no disk budget |
| [cargo-broom](https://github.com/casoon/broom) | Tests `<target>/.cargo-lock`. Cargo never creates that file (the lock is per profile), so its "build running" check can never fire. Its test passes because the test creates the file at the wrong path |
| [rldyour-cleaner](https://github.com/NDDev-OpenNetwork/rldyour-cleaner) | Same lock path, same result; process and freshness checks remain. AGPL-3.0, three days old at the time |
| [worktree-gc](https://github.com/wycats/worktree-gc) | The most complete: correct per-profile locks, libproc ownership, a pressure controller with hysteresis, APFS private-size accounting, quarantine before delete. Three findings on a real machine: it matches build directories by name, and its dry run planned to delete `node_modules/.pnpm/airbnb-js-shims@2.2.1/node_modules/airbnb-js-shims/target`, which holds that package's `es2015.js`–`es5.js`; its pressure mode exists only on `main` (the published crate is 0.1.0 from June); it sees only directories inside git repositories |
| storage_ballast_helper | Its licence grants no rights to parties acting for certain AI companies and forbids use in "automated systems". Not usable in an agent-run pipeline |
| Cargo itself | Automatic GC (1.88+) covers `~/.cargo` only. Build-directory GC is accepted (rust-lang/cargo#13136) with no schedule; `cargo clean gc` is nightly-only |

`target-gc` keeps worktree-gc's design where it was right — per-profile locks,
process ownership, hysteresis, rename-aside before delete, measuring with `df`
— and differs where it was wrong for this job: Cargo's tag rather than a name,
any directory rather than only git repositories, and service definitions and
config files as protection.

## Limits

- **A deletion inside a build directory restarts its idle clock.** Activity is
  read from directory mtimes, and removing a file moves its directory's mtime
  as much as creating one does. Running `cargo sweep` on a directory makes it
  look freshly built for the next `max_idle_days`. `target-gc` sweeps rustc
  scratch only in recently built directories for this reason.
- **A cwd anywhere in the project holds its build directory.** An idle shell
  left open in a large project keeps its `target/` indefinitely. That is the
  conservative direction; `target-gc scan` names the process.
- **Unix only.** macOS and Linux; the process snapshot is `lsof` on macOS and
  `/proc` on Linux. Not built for Windows.
- **A dry run from your terminal can see less than the scheduled job.** macOS
  privacy protection decides per process whether `~/Documents`, `~/Desktop` and
  `~/Downloads` are readable. On the first install, a dry run from an agent's
  shell reported three unreadable directories and nothing to evict; the launchd
  job read all of them and evicted three build directories idle for 30 days in
  `~/Documents`. Read `last-run.json` after the first scheduled run, and add
  those folders to `skip` if they should never be touched.
