use gates_config::Config;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const VERSION: &str = "0.1.0";
const DEFAULT_CONTEXT: &str = "wolven/gates";
const MAX_COMPILE_WAIT: u64 = 240;
const CHECK_TIMEOUT: u64 = 300;

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("--version") => {
            println!("task-gates {}", VERSION);
            return;
        }
        Some("--plan") => {
            if let Err(e) = plan(&args[2..]) {
                eprintln!("error: {}", e);
                std::process::exit(2);
            }
            return;
        }
        None | Some("run") => {
            if let Err(e) = run(&args[1..]) {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("usage: task-gates [--version|--plan|run]");
            std::process::exit(2);
        }
    }
}

fn plan(_args: &[String]) -> Result<(), String> {
    let repo_root = find_git_root()?;
    let config_path = repo_root.join(".gates.toml");

    let config = Config::read(&config_path)
        .map_err(|e| format!("config parse error: {}", e))?
        .unwrap_or_default();

    if config.is_empty() {
        println!("no .gates.toml found; running all checks");
        return Ok(());
    }

    let changed = get_changed_files(&repo_root)?;
    let is_all = env::var("GATES_ALL").is_ok() || changed.is_none();

    let context = config.string("context").unwrap_or(DEFAULT_CONTEXT);
    println!("context: {}", context);

    let check_elements = config.array_elements("check");
    if check_elements.is_empty() {
        println!("no checks configured");
        return Ok(());
    }

    for element in check_elements {
        let name = config
            .string(&format!("{}.name", element))
            .ok_or_else(|| "check missing name".to_string())?;

        if is_all {
            println!("  run {}", name);
            continue;
        }

        let paths = config.list(&format!("{}.paths", element));
        if let Some(patterns) = paths {
            if let Some(ref changed_files) = changed {
                if matches_any(&patterns, changed_files) {
                    println!("  run {} (matches changed files)", name);
                } else {
                    println!("  skip {} (no changed files match)", name);
                }
            }
        } else {
            println!("  run {} (no path filter)", name);
        }
    }

    Ok(())
}

fn run(_args: &[String]) -> Result<(), String> {
    let repo_root = find_git_root()?;
    let config_path = repo_root.join(".gates.toml");

    let config = Config::read(&config_path)
        .map_err(|e| format!("config parse error: {}", e))?
        .unwrap_or_default();

    if config.is_empty() {
        return Err("no .gates.toml found".to_string());
    }

    let check_elements = config.array_elements("check");
    if check_elements.is_empty() {
        return Err("no checks configured".to_string());
    }

    let service_elements = config.array_elements("service");
    let mut services: HashMap<String, ServiceManager> = HashMap::new();

    let changed = get_changed_files(&repo_root)?;
    let is_all = env::var("GATES_ALL").is_ok() || changed.is_none();

    let mut passed = 0;
    let mut failed = 0;

    for element in check_elements {
        let name = config
            .string(&format!("{}.name", element))
            .ok_or_else(|| "check missing name".to_string())?
            .to_string();

        // Check if this check should run
        if !is_all {
            let paths = config.list(&format!("{}.paths", element));
            if let Some(patterns) = paths {
                if let Some(ref changed_files) = changed {
                    if !matches_any(&patterns, changed_files) {
                        continue;
                    }
                }
            }
        }

        // Start any needed services
        let needed_services = config
            .list(&format!("{}.needs", element))
            .unwrap_or_default();
        for svc_name in &needed_services {
            if !services.contains_key(svc_name) {
                for svc_elem in &service_elements {
                    if let Some(s_name) = config.string(&format!("{}.name", svc_elem)) {
                        if s_name == svc_name {
                            let run_cmd = config
                                .list(&format!("{}.run", svc_elem))
                                .ok_or_else(|| "service missing run".to_string())?;
                            let ready_tcp = config.string(&format!("{}.ready_tcp", svc_elem));
                            let mut mgr = ServiceManager::new(
                                svc_name.clone(),
                                run_cmd,
                                ready_tcp.map(|s| s.to_string()),
                            );
                            mgr.start(&repo_root)?;
                            services.insert(svc_name.clone(), mgr);
                            break;
                        }
                    }
                }
            }
        }

        // Run the check
        let run_cmd = config
            .list(&format!("{}.run", element))
            .ok_or_else(|| "check missing run".to_string())?;
        let env_vars = config.list(&format!("{}.env", element)).unwrap_or_default();

        wait_for_compile_lock(&run_cmd)?;

        match run_check(&repo_root, &name, &run_cmd, &env_vars) {
            Ok(duration) => {
                println!("ok {} ({}s)", name, duration.as_secs());
                passed += 1;
            }
            Err(e) => {
                eprintln!("{}", e);
                failed += 1;
            }
        }
    }

    for (_, mut mgr) in services {
        let _ = mgr.stop();
    }

    let _ = post_status(&repo_root, &config, passed, failed);

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn find_git_root() -> Result<PathBuf, String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .map_err(|e| format!("git not found: {}", e))?;

    if !output.status.success() {
        return Err("not in a git repository".to_string());
    }

    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

fn get_changed_files(repo_root: &Path) -> Result<Option<Vec<String>>, String> {
    let _ = Command::new("git")
        .current_dir(repo_root)
        .arg("fetch")
        .arg("--quiet")
        .arg("origin")
        .arg("main")
        .output();

    let base = Command::new("git")
        .current_dir(repo_root)
        .arg("merge-base")
        .arg("origin/main")
        .arg("HEAD")
        .output()
        .map_err(|e| format!("git merge-base failed: {}", e))?;

    if !base.status.success() {
        return Ok(None);
    }

    let base_sha = String::from_utf8_lossy(&base.stdout).trim().to_string();
    let diff_output = Command::new("git")
        .current_dir(repo_root)
        .arg("diff")
        .arg("--name-only")
        .arg(&base_sha)
        .output()
        .map_err(|e| format!("git diff failed: {}", e))?;

    let mut files = String::from_utf8_lossy(&diff_output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let untracked = Command::new("git")
        .current_dir(repo_root)
        .arg("ls-files")
        .arg("--others")
        .arg("--exclude-standard")
        .output()
        .map_err(|e| format!("git ls-files failed: {}", e))?;

    if untracked.status.success() {
        for line in String::from_utf8_lossy(&untracked.stdout).lines() {
            files.push(line.to_string());
        }
    }

    Ok(Some(files))
}

fn matches_any(patterns: &[String], files: &[String]) -> bool {
    for pattern in patterns {
        for file in files {
            if glob_match(pattern, file) {
                return true;
            }
        }
    }
    false
}

fn glob_match(pattern: &str, path: &str) -> bool {
    if pattern == "**" {
        return true;
    }

    if pattern.ends_with("/**") {
        let dir = &pattern[..pattern.len() - 3];
        return path.starts_with(dir)
            && (path.len() == dir.len() || path.as_bytes()[dir.len()] == b'/');
    }

    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split('/').collect();
        return glob_match_parts(&parts, path);
    }

    glob_simple_match(pattern, path)
}

fn glob_match_parts(parts: &[&str], path: &str) -> bool {
    let path_parts: Vec<&str> = path.split('/').collect();
    let (mut pattern_idx, mut path_idx) = (0, 0);

    while pattern_idx < parts.len() && path_idx < path_parts.len() {
        let part = parts[pattern_idx];
        if part == "**" {
            if pattern_idx + 1 >= parts.len() {
                return true;
            }
            for i in path_idx..=path_parts.len() {
                if glob_match_parts(&parts[pattern_idx + 1..], &path_parts[i..].join("/")) {
                    return true;
                }
            }
            return false;
        }
        if !glob_simple_match(part, path_parts[path_idx]) {
            return false;
        }
        pattern_idx += 1;
        path_idx += 1;
    }

    pattern_idx >= parts.len() && path_idx >= path_parts.len()
}

fn glob_simple_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    glob_match_recursive(&pattern_chars, &text_chars, 0, 0)
}

fn glob_match_recursive(pattern: &[char], text: &[char], p_idx: usize, t_idx: usize) -> bool {
    if p_idx >= pattern.len() {
        return t_idx >= text.len();
    }

    if pattern[p_idx] == '*' {
        if p_idx + 1 >= pattern.len() {
            return true;
        }
        for i in t_idx..=text.len() {
            if glob_match_recursive(pattern, text, p_idx + 1, i) {
                return true;
            }
        }
        return false;
    }

    if t_idx >= text.len() {
        return false;
    }

    if pattern[p_idx] == '?' || pattern[p_idx] == text[t_idx] {
        return glob_match_recursive(pattern, text, p_idx + 1, t_idx + 1);
    }

    false
}

fn wait_for_compile_lock(run_cmd: &[String]) -> Result<(), String> {
    if run_cmd.is_empty() {
        return Ok(());
    }

    let first = &run_cmd[0];
    let needs_wait = matches!(first.as_str(), "cargo" | "dx" | "rustc" | "bun");

    if !needs_wait {
        return Ok(());
    }

    let start = Instant::now();
    while start.elapsed().as_secs() < MAX_COMPILE_WAIT {
        let output = Command::new("pgrep")
            .arg("-x")
            .arg("rustc")
            .output()
            .map_err(|e| format!("pgrep failed: {}", e))?;

        if !output.status.success() {
            return Ok(());
        }

        thread::sleep(Duration::from_secs(5));
    }

    Err("another compile held the machine".to_string())
}

fn run_check(
    repo_root: &Path,
    name: &str,
    run_cmd: &[String],
    env_vars: &[String],
) -> Result<Duration, String> {
    if run_cmd.is_empty() {
        return Err("check command is empty".to_string());
    }

    let mut cmd = Command::new(&run_cmd[0]);
    cmd.current_dir(repo_root);

    for arg in &run_cmd[1..] {
        cmd.arg(arg);
    }

    for env_var in env_vars {
        if let Some((key, value)) = env_var.split_once('=') {
            cmd.env(key, value);
        }
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let start = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn check: {}", e))?;

    let status = wait_with_timeout(&mut child, Duration::from_secs(CHECK_TIMEOUT))?;

    if status.success() {
        Ok(start.elapsed())
    } else {
        Err(format!("FAIL {} ({}s)", name, start.elapsed().as_secs()))
    }
}

fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, String> {
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("TIMEOUT (>{}s)", timeout.as_secs()));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(format!("wait failed: {}", e)),
        }
    }
}

struct ServiceManager {
    name: String,
    cmd: Vec<String>,
    ready_tcp: Option<String>,
    process: Option<Child>,
}

impl ServiceManager {
    fn new(name: String, cmd: Vec<String>, ready_tcp: Option<String>) -> Self {
        ServiceManager {
            name,
            cmd,
            ready_tcp,
            process: None,
        }
    }

    fn start(&mut self, repo_root: &Path) -> Result<(), String> {
        if self.cmd.is_empty() {
            return Err("service command is empty".to_string());
        }

        let mut cmd = Command::new(&self.cmd[0]);
        cmd.current_dir(repo_root);

        for arg in &self.cmd[1..] {
            cmd.arg(arg);
        }

        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        let child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn service {}: {}", self.name, e))?;

        self.process = Some(child);

        if let Some(ref tcp) = self.ready_tcp {
            self.wait_for_port(tcp)?;
        }

        Ok(())
    }

    fn wait_for_port(&self, addr: &str) -> Result<(), String> {
        let start = Instant::now();
        while start.elapsed().as_secs() < 60 {
            if std::net::TcpStream::connect(addr).is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(500));
        }
        Err(format!(
            "service {} did not become ready on {}",
            self.name, addr
        ))
    }

    fn stop(&mut self) -> Result<(), String> {
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
        Ok(())
    }
}

fn post_status(
    repo_root: &Path,
    config: &Config,
    passed: usize,
    failed: usize,
) -> Result<(), String> {
    let status_output = Command::new("git")
        .current_dir(repo_root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .map_err(|e| format!("git status failed: {}", e))?;

    if !status_output.stdout.is_empty() {
        return Ok(());
    }

    let sha_output = Command::new("git")
        .current_dir(repo_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|e| format!("git rev-parse failed: {}", e))?;

    let sha = String::from_utf8_lossy(&sha_output.stdout)
        .trim()
        .to_string();

    let repo_output = Command::new("gh")
        .current_dir(repo_root)
        .arg("repo")
        .arg("view")
        .arg("--json")
        .arg("nameWithOwner")
        .arg("--jq")
        .arg(".nameWithOwner")
        .output()
        .map_err(|e| format!("gh repo view failed: {}", e))?;

    if !repo_output.status.success() {
        return Ok(());
    }

    let repo = String::from_utf8_lossy(&repo_output.stdout)
        .trim()
        .to_string();

    let context = config.string("context").unwrap_or(DEFAULT_CONTEXT);
    let state = if failed == 0 { "success" } else { "failure" };
    let description = if failed == 0 {
        format!("task gates: {} checks passed", passed)
    } else {
        format!(
            "task gates: {} of {} checks failed",
            failed,
            passed + failed
        )
    };

    let _ = Command::new("gh")
        .current_dir(repo_root)
        .arg("api")
        .arg("-X")
        .arg("POST")
        .arg(&format!("repos/{}/statuses/{}", repo, sha))
        .arg("-f")
        .arg(&format!("state={}", state))
        .arg("-f")
        .arg(&format!("context={}", context))
        .arg("-f")
        .arg(&format!("description={}", description))
        .output();

    Ok(())
}
