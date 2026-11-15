use crate::db::Database;
use crate::defaults;
use anyhow::Result;
use notify::RecommendedWatcher;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct PendingTurn {
    pub story_id: String,
    pub turn_index: i64,
    pub mode: String,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub worlds_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub db_path: PathBuf,
    pub db: Database,
    /// True for the portable build (data lives next to the exe).
    pub portable: bool,
    /// Legacy %APPDATA% directory we copied from, if the user accepted.
    pub migrated_from: Option<String>,
    /// Factory files we could not auto-upgrade because the user edited them.
    pub needs_merge: Mutex<Vec<String>>,
    /// Kept alive so the filesystem watcher is not dropped.
    pub watcher: Mutex<Option<RecommendedWatcher>>,
    pub cancels: Mutex<HashMap<String, CancellationToken>>,
    pub pending: Mutex<HashMap<String, PendingTurn>>,
}

impl AppState {
    pub fn new(data_dir: PathBuf, migrated_from: Option<String>) -> Result<Self> {
        let config_dir = data_dir.join("config");
        let worlds_dir = data_dir.join("worlds");
        let backups_dir = data_dir.join("backups");
        let db_path = data_dir.join("novel.db");

        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&worlds_dir)?;
        std::fs::create_dir_all(&backups_dir)?;

        // First run only: materialise the shipped defaults as editable files.
        for (rel, content) in defaults::all() {
            let target = config_dir.join(rel);
            if !target.exists() {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, content)?;
            }
        }

        let db = Database::open(&db_path)?;

        Ok(Self {
            data_dir,
            config_dir,
            worlds_dir,
            backups_dir,
            db_path,
            db,
            portable: true,
            migrated_from,
            needs_merge: Mutex::new(Vec::new()),
            watcher: Mutex::new(None),
            cancels: Mutex::new(HashMap::new()),
            pending: Mutex::new(HashMap::new()),
        })
    }

    pub fn now() -> String {
        chrono::Local::now().to_rfc3339()
    }

    pub fn new_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_initialises_db_and_config() {
        let dir = std::env::temp_dir()
            .join(format!("novel-init-test-{}", uuid::Uuid::new_v4()));
        let st = AppState::new(dir.clone(), None).unwrap();
        assert!(st.config_dir.join("engines.yaml").exists());
        assert!(st.config_dir.join("policies/local.yaml").exists());
        assert!(st.config_dir.join("templates/system.local.md").exists());
        assert!(st.db_path.exists());
        let conn = st.db.lock().unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name IN
                 ('lore_fts','lore_chunks','passages','state_card')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 4);
        drop(conn);
        drop(st);
        let _ = std::fs::remove_dir_all(dir);
    }
}
