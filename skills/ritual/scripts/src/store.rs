use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use allsource_core::embedded::{Config, EmbeddedCore, IngestEvent, Query};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::feedback::{EVENT_PREFIX, Feedback};

const LOCK_WAIT: Duration = Duration::from_secs(10);

/// One recorded feedback event, as read back from the store.
#[derive(Debug, Clone, Serialize)]
pub struct Record {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub feedback: Feedback,
}

/// The store directory: `$RITUAL_FEEDBACK_DIR`, else `~/.ritual/feedback`.
pub fn default_dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("RITUAL_FEEDBACK_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").ok_or("HOME is not set; pass --store")?;
    Ok(PathBuf::from(home).join(".ritual").join("feedback"))
}

/// An exclusive advisory lock, released when dropped.
pub struct FileLock(#[allow(dead_code)] File);

/// Takes the lock at `path`, waiting up to `wait`; `None` when another process holds it.
pub fn exclusive(path: &Path, wait: Duration) -> Result<Option<FileLock>, String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let file = File::create(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let deadline = Instant::now() + wait;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(Some(FileLock(file))),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Error(e)) => return Err(format!("lock {}: {e}", path.display())),
        }
    }
}

/// The embedded core has no cross-process lock of its own, and two sessions can finish
/// a step at once.
fn lock(dir: &Path) -> Result<FileLock, String> {
    let path = dir.join("write.lock");
    exclusive(&path, LOCK_WAIT)?.ok_or_else(|| {
        format!(
            "{} is held by another ritual-feedback process",
            path.display()
        )
    })
}

async fn open(dir: &Path) -> Result<EmbeddedCore, String> {
    let config = Config::builder()
        .data_dir(dir.join("allsource"))
        .build()
        .map_err(|e| e.to_string())?;
    EmbeddedCore::open(config).await.map_err(|e| e.to_string())
}

pub fn append(dir: &Path, feedback: &Feedback) -> Result<(), String> {
    feedback.validate()?;
    let _lock = lock(dir)?;
    runtime()?.block_on(async {
        let core = open(dir).await?;
        let payload = serde_json::to_value(feedback).map_err(|e| e.to_string())?;
        let result = core
            .ingest(IngestEvent {
                entity_id: &feedback.entity_id(),
                event_type: &feedback.event_type(),
                payload,
                metadata: Some(serde_json::json!({
                    "source": "ritual-feedback",
                    "version": env!("CARGO_PKG_VERSION"),
                })),
                tenant_id: None,
            })
            .await
            .map_err(|e| e.to_string());
        // The WAL is fsynced on ingest, so a failed shutdown does not lose the event;
        // reporting it as a failed record would invite a duplicate.
        if let Err(e) = core.shutdown().await {
            eprintln!("ritual-feedback: store shutdown after a recorded event: {e}");
        }
        result
    })
}

/// Every feedback event in record-time order. Events that do not parse as the
/// current schema are skipped and counted, never silently dropped.
pub fn read_all(dir: &Path) -> Result<(Vec<Record>, usize), String> {
    let _lock = lock(dir)?;
    let events = runtime()?.block_on(async {
        let core = open(dir).await?;
        let events = core
            .query(Query::new().event_type_prefix(EVENT_PREFIX))
            .await
            .map_err(|e| e.to_string());
        core.shutdown().await.map_err(|e| e.to_string())?;
        events
    })?;
    let mut records = Vec::with_capacity(events.len());
    let mut unreadable = 0;
    for event in events {
        match serde_json::from_value::<Feedback>(event.payload) {
            Ok(feedback) => records.push(Record {
                id: event.id.to_string(),
                timestamp: event.timestamp,
                feedback,
            }),
            Err(_) => unreadable += 1,
        }
    }
    records.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));
    Ok((records, unreadable))
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())
}
