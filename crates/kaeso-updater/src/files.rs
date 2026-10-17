use std::io::Write;
use std::path::{Path, PathBuf};

use kaeso_update::manifest::Service;
use kaeso_update::protocol::{Job, Maintenance, Step};
use serde::{Deserialize, Serialize};

pub const MAINTENANCE_FILE: &str = "active";
pub const STATE_FILE: &str = "job.json";

fn write_atomically(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let temp = dir.join(format!(".{name}.tmp"));
    {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&temp, dir.join(name))
}

#[derive(Clone, Debug)]
pub struct MaintenanceFlag {
    dir: PathBuf,
}

impl MaintenanceFlag {
    pub fn new(dir: impl Into<PathBuf>) -> MaintenanceFlag {
        MaintenanceFlag { dir: dir.into() }
    }

    pub fn set(&self, step: Step) -> std::io::Result<()> {
        let bytes =
            serde_json::to_vec(&Maintenance::updating(step)).map_err(std::io::Error::other)?;
        write_atomically(&self.dir, MAINTENANCE_FILE, &bytes)
    }

    pub fn clear(&self) -> std::io::Result<()> {
        match std::fs::remove_file(self.dir.join(MAINTENANCE_FILE)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    pub fn is_set(&self) -> bool {
        self.dir.join(MAINTENANCE_FILE).exists()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switched {
    pub service: Service,
    pub name: String,
    pub old_id: String,
    pub new_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub job: Job,
    pub switched: Vec<Switched>,
}

#[derive(Clone, Debug)]
pub struct StateFile {
    dir: PathBuf,
}

impl StateFile {
    pub fn new(dir: impl Into<PathBuf>) -> StateFile {
        StateFile { dir: dir.into() }
    }

    pub fn save(&self, saved: &Saved) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(saved).map_err(std::io::Error::other)?;
        write_atomically(&self.dir, STATE_FILE, &bytes)
    }

    pub fn load(&self) -> Option<Saved> {
        let bytes = std::fs::read(self.dir.join(STATE_FILE)).ok()?;
        serde_json::from_slice(&bytes)
            .inspect_err(
                |e| tracing::warn!(error = %e, "the saved update does not parse; it is ignored"),
            )
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeso_update::protocol::JobState;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kaeso-files-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_maintenance_flag_names_the_step_and_goes_away() {
        let d = dir("flag");
        let flag = MaintenanceFlag::new(&d);
        assert!(!flag.is_set());
        flag.set(Step::Migrating).unwrap();
        assert!(flag.is_set());
        let text = std::fs::read_to_string(d.join(MAINTENANCE_FILE)).unwrap();
        assert_eq!(text, r#"{"state":"updating","step":"migrating"}"#);
        flag.clear().unwrap();
        flag.clear().unwrap();
        assert!(!flag.is_set());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn saved_updates_round_trip_and_garbage_is_ignored() {
        let d = dir("state");
        let state = StateFile::new(&d);
        assert_eq!(state.load(), None);
        let saved = Saved {
            job: Job {
                id: "upd_01k7a3f8m2xq4v9c5r6t8w0yz1".into(),
                from: "0.2.0".into(),
                to: "0.2.1".into(),
                state: JobState::Running,
                step: Step::Switching,
                reason: None,
                detail: None,
                started_at: time::OffsetDateTime::UNIX_EPOCH,
                finished_at: None,
            },
            switched: vec![Switched {
                service: Service::Gateway,
                name: "kaeso-gateway-1".into(),
                old_id: "old".into(),
                new_id: Some("new".into()),
            }],
        };
        state.save(&saved).unwrap();
        assert_eq!(state.load(), Some(saved));
        std::fs::write(d.join(STATE_FILE), b"{").unwrap();
        assert_eq!(state.load(), None);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
