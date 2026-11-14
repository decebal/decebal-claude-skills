use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

pub const LABEL: &str = "com.ritual-feedback.weekly";
/// Seconds between calls. Most calls find nothing to do and return at once; launchd
/// fires a missed interval on wake, so a sleeping Mac catches up.
pub const INTERVAL_SECONDS: u32 = 900;

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn plist(args: &[String], env_path: &str, home: &Path, log: &Path) -> String {
    let program = args
        .iter()
        .map(|a| format!("    <string>{}</string>\n", xml_escape(a)))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
{program}  </array>
  <key>StartInterval</key>
  <integer>{INTERVAL_SECONDS}</integer>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>{path}</string>
    <key>HOME</key>
    <string>{home}</string>
  </dict>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#,
        path = xml_escape(env_path),
        home = xml_escape(&home.display().to_string()),
        log = xml_escape(&log.display().to_string()),
    )
}

pub fn plist_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// Writes the LaunchAgent and (re)loads it into the user's GUI domain.
pub fn install(body: &str, home: &Path) -> Result<PathBuf, String> {
    let path = plist_path(home);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    let uid = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|e| format!("id -u: {e}"))?;
    let domain = format!("gui/{}", String::from_utf8_lossy(&uid.stdout).trim());
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{domain}/{LABEL}")])
        .output();
    // bootout returns before the old job is torn down, so the first bootstrap can race it.
    let mut last = String::new();
    for _ in 0..10 {
        let boot = Command::new("launchctl")
            .args(["bootstrap", &domain])
            .arg(&path)
            .output()
            .map_err(|e| format!("launchctl: {e}"))?;
        if boot.status.success() {
            return Ok(path);
        }
        last = String::from_utf8_lossy(&boot.stderr).trim().to_string();
        thread::sleep(Duration::from_millis(500));
    }
    Err(format!(
        "launchctl bootstrap {domain} {}: {last}",
        path.display()
    ))
}
