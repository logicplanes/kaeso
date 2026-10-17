use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub const VERSION: u32 = 1;
pub const TOKEN_MIN_LEN: usize = 32;

macro_rules! names {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            pub fn parse(s: &str) -> Option<$name> {
                $name::ALL.iter().copied().find(|v| v.as_str() == s)
            }
        }
    };
}

names!(Step {
    Verifying => "verifying",
    Downloading => "downloading",
    Switching => "switching",
    Migrating => "migrating",
    Online => "online",
});

names!(JobState {
    Running => "running",
    Succeeded => "succeeded",
    Failed => "failed",
    RolledBack => "rolled_back",
    RollbackFailed => "rollback_failed",
    NeedsAttention => "needs_attention",
});

names!(Reason {
    ManifestUnavailable => "manifest_unavailable",
    ManifestInvalid => "manifest_invalid",
    ManualSteps => "manual_steps",
    Major => "major",
    NotNewer => "not_newer",
    UpdaterOutdated => "updater_outdated",
    ComposeChanged => "compose_changed",
    ComposeUnwritable => "compose_unwritable",
    ServicesNotFound => "services_not_found",
    PullFailed => "pull_failed",
    Unverified => "unverified",
    DockerUnavailable => "docker_unavailable",
    Unhealthy => "unhealthy",
    Interrupted => "interrupted",
    UpdaterUnreachable => "updater_unreachable",
    UpdaterRefused => "updater_refused",
    StateUnwritable => "state_unwritable",
    Migrated => "migrated",
});

impl JobState {
    pub fn is_finished(self) -> bool {
        self != JobState::Running
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRequest {
    pub id: String,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub from: String,
    pub to: String,
    pub state: JobState,
    pub step: Step,
    pub reason: Option<Reason>,
    pub detail: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub version: String,
    pub protocol: u32,
    pub docker: bool,
    pub job: Option<Job>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Maintenance {
    pub state: String,
    pub step: Step,
}

impl Maintenance {
    pub fn updating(step: Step) -> Maintenance {
        Maintenance {
            state: "updating".into(),
            step,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_parses_back_and_unknown_names_do_not() {
        for s in Step::ALL {
            assert_eq!(Step::parse(s.as_str()), Some(*s));
        }
        for s in JobState::ALL {
            assert_eq!(JobState::parse(s.as_str()), Some(*s));
        }
        for r in Reason::ALL {
            assert_eq!(Reason::parse(r.as_str()), Some(*r));
            assert_eq!(
                serde_json::to_value(r).unwrap(),
                serde_json::json!(r.as_str())
            );
        }
        assert_eq!(Step::parse("rebooting"), None);
        assert_eq!(JobState::parse("paused"), None);
        assert_eq!(Reason::parse("gremlins"), None);
    }

    #[test]
    fn only_running_jobs_are_unfinished() {
        assert!(!JobState::Running.is_finished());
        for s in JobState::ALL.iter().filter(|s| **s != JobState::Running) {
            assert!(s.is_finished(), "{s:?}");
        }
    }

    #[test]
    fn jobs_travel_as_json_with_rfc3339_times() {
        let job = Job {
            id: "upd_01k7a3f8m2xq4v9c5r6t8w0yz1".into(),
            from: "0.2.0".into(),
            to: "0.2.1".into(),
            state: JobState::Running,
            step: Step::Verifying,
            reason: None,
            detail: None,
            started_at: OffsetDateTime::from_unix_timestamp(1_792_193_616).unwrap(),
            finished_at: None,
        };
        let value = serde_json::to_value(&job).unwrap();
        assert_eq!(value["step"], "verifying");
        assert_eq!(value["started_at"], "2026-10-16T23:33:36Z");
        assert_eq!(serde_json::from_value::<Job>(value).unwrap(), job);
    }

    #[test]
    fn the_maintenance_file_says_only_the_state_and_the_step() {
        let value = serde_json::to_value(Maintenance::updating(Step::Migrating)).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "state": "updating", "step": "migrating" })
        );
    }
}
