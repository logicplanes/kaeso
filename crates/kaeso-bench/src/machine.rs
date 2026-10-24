use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Machine {
    pub(crate) cpu: String,
    pub(crate) cores: usize,
    pub(crate) memory_bytes: Option<u64>,
    pub(crate) os: String,
    pub(crate) arch: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Build {
    pub(crate) version: String,
    pub(crate) commit: String,
    pub(crate) profile: String,
}

pub(crate) fn machine() -> Machine {
    Machine {
        cpu: cpu().unwrap_or_else(|| "unknown".to_owned()),
        cores: std::thread::available_parallelism().map_or(0, usize::from),
        memory_bytes: memory(),
        os: os(),
        arch: std::env::consts::ARCH.to_owned(),
    }
}

pub(crate) fn build() -> Build {
    Build {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: commit().unwrap_or_else(|| "unknown".to_owned()),
        profile: if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
        .to_owned(),
    }
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn cpu() -> Option<String> {
    if cfg!(target_os = "macos") {
        return output("sysctl", &["-n", "machdep.cpu.brand_string"]);
    }
    let info = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    info.lines()
        .find_map(|l| l.strip_prefix("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, name)| name.trim().to_owned())
}

fn memory() -> Option<u64> {
    if cfg!(target_os = "macos") {
        return output("sysctl", &["-n", "hw.memsize"])?.parse().ok();
    }
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kib: u64 = info
        .lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(kib * 1024)
}

fn os() -> String {
    let version = if cfg!(target_os = "macos") {
        output("sw_vers", &["-productVersion"])
    } else {
        output("uname", &["-r"])
    };
    match version {
        Some(v) => format!("{} {v}", std::env::consts::OS),
        None => std::env::consts::OS.to_owned(),
    }
}

fn commit() -> Option<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .into_owned();
    let hash = output("git", &["-C", &dir, "rev-parse", "--short=12", "HEAD"])?;
    let changed = output(
        "git",
        &["-C", &dir, "status", "--porcelain", "--untracked-files=no"],
    )
    .is_some();
    Some(if changed {
        format!("{hash} with local changes")
    } else {
        hash
    })
}

pub(crate) fn rss_bytes(pid: u32) -> Option<u64> {
    let kib: u64 = output("ps", &["-o", "rss=", "-p", &pid.to_string()])?
        .parse()
        .ok()?;
    Some(kib * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_machine_and_this_process_are_described() {
        let m = machine();
        assert!(m.cores > 0);
        assert_ne!(m.cpu, "unknown");
        assert!(m.memory_bytes.is_some_and(|b| b > 0));
        assert!(rss_bytes(std::process::id()).is_some_and(|b| b > 0));
        assert_eq!(build().version, env!("CARGO_PKG_VERSION"));
    }
}
