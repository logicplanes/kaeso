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

// The CPU time, user and system, the process has used so far, all its threads included.
pub(crate) fn cpu_seconds(pid: u32) -> Option<f64> {
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        return proc_stat_seconds(&stat);
    }
    ps_time_seconds(&output("ps", &["-o", "time=", "-p", &pid.to_string()])?)
}

// Fields 14 and 15 of /proc/<pid>/stat count clock ticks, which Linux reports in USER_HZ, a
// fixed 100 per second on every architecture (proc(5)).
fn proc_stat_seconds(stat: &str) -> Option<f64> {
    let (_, after_name) = stat.rsplit_once(')')?;
    let mut fields = after_name.split_whitespace().skip(11);
    let user: u64 = fields.next()?.parse().ok()?;
    let system: u64 = fields.next()?.parse().ok()?;
    Some((user + system) as f64 / 100.0)
}

// `ps -o time=`: `[[dd-]hh:]mm:ss[.cc]`.
fn ps_time_seconds(time: &str) -> Option<f64> {
    let (days, clock) = match time.trim().split_once('-') {
        Some((d, rest)) => (d.parse::<f64>().ok()?, rest),
        None => (0.0, time.trim()),
    };
    let mut seconds = 0.0;
    for part in clock.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(days * 86_400.0 + seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_time_reads_in_both_formats_and_for_this_process() {
        assert_eq!(ps_time_seconds("0:01.25"), Some(1.25));
        assert_eq!(ps_time_seconds("01:02:03"), Some(3723.0));
        assert_eq!(ps_time_seconds("1-00:00:01"), Some(86_401.0));
        assert_eq!(ps_time_seconds("soon"), None);
        let stat = "42 (kaeso bench) S 1 42 42 0 -1 4194304 1 0 0 0 250 50 0 0 20 0 9 0 1 1 1";
        assert_eq!(proc_stat_seconds(stat), Some(3.0));
        assert!(cpu_seconds(std::process::id()).is_some());
    }

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
