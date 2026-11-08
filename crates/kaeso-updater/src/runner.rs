use std::time::Duration;

use serde_json::{Value, json};

pub const JOB_LABEL: &str = "app.kaeso.updater.runner";
pub const TASK_LABEL: &str = "app.kaeso.updater.task";
pub const ALIAS: &str = "updater-runner";
pub const LOG_LINES: u32 = 20;
const PROXY: &str = "tcp://docker-proxy:2375";
const MEMORY: u64 = 256 << 20;
const PIDS: u64 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Stop,
    Pull,
    Up(Duration),
    Replace(Duration),
}

impl Task {
    pub fn name(self) -> &'static str {
        match self {
            Task::Stop => "stop",
            Task::Pull => "pull",
            Task::Up(_) => "up",
            Task::Replace(_) => "replace",
        }
    }

    pub fn args(self) -> Vec<String> {
        let mut args = vec!["compose".to_owned(), self.name().to_owned()];
        if let Task::Up(wait) | Task::Replace(wait) = self {
            args.push(wait.as_secs().max(1).to_string());
        }
        args
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub image: String,
    pub dir: String,
    pub project: String,
    pub network: String,
}

// The runner reads the install's folder at the path Docker knows it by, so the relative paths in
// compose.yaml resolve to the host paths docker-proxy's bind-mount check compares.
pub fn spec(place: &Place, task: Task, job: &str) -> Value {
    json!({
        "Image": place.image,
        "Cmd": task.args(),
        "User": "0:0",
        "WorkingDir": place.dir,
        "Env": [
            format!("DOCKER_HOST={PROXY}"),
            format!("COMPOSE_PROJECT_NAME={}", place.project),
            "HOME=/tmp",
        ],
        "Labels": { JOB_LABEL: job, TASK_LABEL: task.name() },
        "HostConfig": {
            "Binds": [format!("{0}:{0}:ro", place.dir)],
            "CapDrop": ["ALL"],
            "CapAdd": ["DAC_READ_SEARCH"],
            "SecurityOpt": ["no-new-privileges:true"],
            "ReadonlyRootfs": true,
            "Tmpfs": { "/tmp": "rw,noexec,nosuid,size=16m" },
            "Memory": MEMORY,
            "PidsLimit": PIDS,
            "NetworkMode": place.network,
            "RestartPolicy": { "Name": "no" },
            "LogConfig": { "Type": "json-file", "Config": { "max-size": "1m" } },
        },
        "NetworkingConfig": {
            "EndpointsConfig": { place.network.as_str(): { "Aliases": [ALIAS] } }
        },
    })
}

pub fn last_lines(logs: &str, count: usize) -> String {
    let lines: Vec<&str> = logs
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines
        .get(lines.len().saturating_sub(count)..)
        .unwrap_or_default()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place() -> Place {
        Place {
            image: format!(
                "ghcr.io/logicplanes/kaeso-updater@sha256:{}",
                "a".repeat(64)
            ),
            dir: "/srv/kaeso".into(),
            project: "kaeso".into(),
            network: "kaeso_docker".into(),
        }
    }

    #[test]
    fn each_task_runs_the_compose_subcommand_with_its_arguments() {
        assert_eq!(Task::Stop.args(), ["compose", "stop"]);
        assert_eq!(Task::Pull.args(), ["compose", "pull"]);
        assert_eq!(
            Task::Up(Duration::from_secs(600)).args(),
            ["compose", "up", "600"]
        );
        assert_eq!(
            Task::Replace(Duration::from_millis(10)).args(),
            ["compose", "replace", "1"]
        );
    }

    #[test]
    fn the_runner_reads_the_install_at_its_own_path_and_reaches_only_the_proxy() {
        let spec = spec(&place(), Task::Stop, "upd_1");
        assert_eq!(spec["WorkingDir"], "/srv/kaeso");
        assert_eq!(
            spec["HostConfig"]["Binds"],
            json!(["/srv/kaeso:/srv/kaeso:ro"])
        );
        assert_eq!(spec["HostConfig"]["NetworkMode"], "kaeso_docker");
        assert_eq!(
            spec["NetworkingConfig"]["EndpointsConfig"]["kaeso_docker"]["Aliases"],
            json!([ALIAS])
        );
        assert_eq!(
            spec["Labels"],
            json!({ JOB_LABEL: "upd_1", TASK_LABEL: "stop" })
        );
        let env: Vec<&str> = spec["Env"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(env.contains(&"DOCKER_HOST=tcp://docker-proxy:2375"));
        assert!(env.contains(&"COMPOSE_PROJECT_NAME=kaeso"));
        assert!(
            !env.iter().any(|e| e.starts_with("DOCKER_API_VERSION")),
            "Compose picks the version the user's own Compose would, so their labels agree"
        );
        assert_eq!(crate::guard::runner(&spec), Ok(()));
    }

    #[test]
    fn the_last_lines_of_a_log_become_one_line() {
        assert_eq!(last_lines("a\n\n b \nc\nd\n", 2), "c | d");
        assert_eq!(last_lines("only\n", 5), "only");
        assert_eq!(last_lines("", 5), "");
    }
}
