use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

pub const COMPOSE_BIN: &str = "/usr/local/bin/docker-compose";
pub const DONE: u8 = 0;
pub const FAILED: u8 = 1;
pub const USAGE: u8 = 2;
pub const REFUSED: u8 = 3;

// The updater and its proxy can't be recreated through the proxy by a runner they started, so
// an update applies every other service and the updater replaces itself last.
const KEPT: [&str; 3] = ["updater", "updater-init", "docker-proxy"];
const SWITCHED: [&str; 2] = ["gateway", "console"];
const KAESO_IMAGES: &str = "ghcr.io/logicplanes/kaeso-";

fn say(text: &str) {
    let _ = writeln!(std::io::stderr(), "{text}");
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

// docker-proxy lets Compose inspect only images named by digest or by id.
pub fn pinned(image: &str) -> bool {
    match image.rsplit_once("@sha256:") {
        Some((name, hex)) => !name.is_empty() && is_hex64(hex),
        None => image.strip_prefix("sha256:").is_some_and(is_hex64),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    pub name: String,
    pub image: Option<String>,
}

pub fn services(config: &[u8]) -> Option<Vec<Service>> {
    let value: Value = serde_json::from_slice(config).ok()?;
    let mut list: Vec<Service> = value
        .get("services")?
        .as_object()?
        .iter()
        .map(|(name, s)| Service {
            name: name.clone(),
            image: s.get("image").and_then(Value::as_str).map(str::to_owned),
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Some(list)
}

pub fn applied(services: &[Service]) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for s in services.iter().filter(|s| !KEPT.contains(&s.name.as_str())) {
        match s.image.as_deref() {
            Some(image) if pinned(image) => names.push(s.name.clone()),
            Some(image) => {
                return Err(format!(
                    "the image of {} (`{image}`) is not pinned by digest; pin it in compose.override.yaml, or update with the installer",
                    s.name
                ));
            }
            None => {
                return Err(format!(
                    "{} has no image; give it one pinned by digest, or update with the installer",
                    s.name
                ));
            }
        }
    }
    Ok(names)
}

pub fn replaced(services: &[Service]) -> Vec<String> {
    services
        .iter()
        .filter(|s| !SWITCHED.contains(&s.name.as_str()) && s.name != "docker-proxy")
        .filter(|s| {
            s.image
                .as_deref()
                .is_some_and(|i| i.starts_with(KAESO_IMAGES) && pinned(i))
        })
        .map(|s| s.name.clone())
        .collect()
}

fn compose(bin: &Path, args: &[String]) -> u8 {
    say(&format!("docker compose {}", args.join(" ")));
    match Command::new(bin).args(args).stdin(Stdio::null()).status() {
        Ok(status) if status.success() => DONE,
        Ok(status) => {
            say(&format!("docker compose ended with {status}"));
            FAILED
        }
        Err(e) => {
            say(&format!("docker compose cannot run: {e}"));
            FAILED
        }
    }
}

fn config(bin: &Path) -> Result<Vec<Service>, u8> {
    let out = Command::new(bin)
        .args(["config", "--format", "json"])
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| {
            say(&format!("docker compose cannot run: {e}"));
            FAILED
        })?;
    if !out.status.success() {
        say("the install's Compose files do not load");
        return Err(FAILED);
    }
    services(&out.stdout).ok_or_else(|| {
        say("docker compose config printed no services");
        FAILED
    })
}

fn seconds(arg: Option<&String>) -> Option<String> {
    arg.filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .cloned()
}

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|a| (*a).to_owned()).collect()
}

pub fn run(bin: &Path, args: &[String]) -> u8 {
    let task = args.first().map(String::as_str);
    let wait = seconds(args.get(1));
    match (task, wait) {
        (Some("stop"), None) if args.len() == 1 => {
            compose(bin, &owned(&["stop", "gateway", "console"]))
        }
        (Some("pull"), None) if args.len() == 1 => match config(bin).map(|s| applied(&s)) {
            Err(code) => code,
            Ok(Err(why)) => {
                say(&why);
                REFUSED
            }
            Ok(Ok(names)) => {
                let mut call = owned(&["pull", "--quiet", "--policy", "missing"]);
                call.extend(names);
                compose(bin, &call)
            }
        },
        (Some("up"), Some(wait)) if args.len() == 2 => match config(bin).map(|s| applied(&s)) {
            Err(code) => code,
            Ok(Err(why)) => {
                say(&why);
                REFUSED
            }
            Ok(Ok(names)) => {
                let mut call = owned(&["up", "--detach", "--wait", "--wait-timeout"]);
                call.push(wait);
                call.extend(names);
                compose(bin, &call)
            }
        },
        (Some("replace"), Some(wait)) if args.len() == 2 => match config(bin) {
            Err(code) => code,
            Ok(services) => {
                let names = replaced(&services);
                if names.is_empty() {
                    say("no service runs a Kaeso image the update hasn't replaced");
                    return DONE;
                }
                let mut call = owned(&["up", "--detach", "--no-deps", "--wait", "--wait-timeout"]);
                call.push(wait);
                call.extend(names);
                compose(bin, &call)
            }
        },
        _ => {
            say("usage: kaeso-updater compose stop | pull | up <seconds> | replace <seconds>");
            USAGE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn s(name: &str, image: Option<&str>) -> Service {
        Service {
            name: name.into(),
            image: image.map(str::to_owned),
        }
    }

    fn install() -> Vec<Service> {
        let kaeso = |n: &str| format!("ghcr.io/logicplanes/kaeso-{n}:40.2.1@sha256:{D}");
        vec![
            s("caddy", Some(&kaeso("console"))),
            s("console", Some(&kaeso("console"))),
            s(
                "docker-proxy",
                Some(&format!("wollomatic/socket-proxy:1.13.2@sha256:{D}")),
            ),
            s("gateway", Some(&kaeso("gateway"))),
            s(
                "postgres",
                Some(&format!("postgres:18.6-alpine@sha256:{D}")),
            ),
            s("updater", Some(&kaeso("updater"))),
            s("updater-init", Some(&kaeso("console"))),
        ]
    }

    #[test]
    fn only_digests_and_image_ids_count_as_pinned() {
        assert!(pinned(&format!("postgres:18@sha256:{D}")));
        assert!(pinned(&format!("ghcr.io/example/app@sha256:{D}")));
        assert!(pinned(&format!("sha256:{D}")));
        assert!(!pinned("postgres:18"));
        assert!(!pinned(&format!("@sha256:{D}")));
        assert!(!pinned(&format!("postgres@sha256:{}", D.to_uppercase())));
        assert!(!pinned("postgres@sha256:abc"));
    }

    #[test]
    fn an_update_applies_every_service_but_the_updater_and_its_proxy() {
        assert_eq!(
            applied(&install()).unwrap(),
            ["caddy", "console", "gateway", "postgres"]
        );
        let mut own = install();
        own.push(s("backup", Some("example/backup:latest")));
        let why = applied(&own).unwrap_err();
        assert!(
            why.contains("backup") && why.contains("compose.override.yaml"),
            "{why}"
        );
        let mut built = install();
        built.push(s("local", None));
        assert!(applied(&built).unwrap_err().contains("local has no image"));
    }

    #[test]
    fn the_updater_replaces_itself_and_every_other_kaeso_service_the_switch_left() {
        let mut list = install();
        list.push(s(
            "caddy-init",
            Some("ghcr.io/logicplanes/kaeso-console:40.2.1"),
        ));
        assert_eq!(replaced(&list), ["caddy", "updater", "updater-init"]);
    }

    #[test]
    fn compose_config_is_read_for_its_services_and_images() {
        let json = br#"{"name":"kaeso","services":{"gateway":{"image":"g"},"web":{"build":{}}}}"#;
        assert_eq!(
            services(json).unwrap(),
            [s("gateway", Some("g")), s("web", None)]
        );
        assert_eq!(services(b"{}"), None);
        assert_eq!(services(b"not json"), None);
    }

    #[cfg(unix)]
    mod with_a_fake_compose {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        struct Fake {
            dir: std::path::PathBuf,
        }

        impl Fake {
            fn new(name: &str, config: &str, exit: u8) -> Fake {
                let dir = std::env::temp_dir()
                    .join(format!("kaeso-compose-cli-{name}-{}", std::process::id()));
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("config.json"), config).unwrap();
                let script = format!(
                    "#!/bin/sh\nif [ \"$1\" = config ]; then cat '{}'; exit 0; fi\necho \"$@\" >> '{}'\nexit {exit}\n",
                    dir.join("config.json").display(),
                    dir.join("calls").display()
                );
                let bin = dir.join("docker-compose");
                std::fs::write(&bin, script).unwrap();
                std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
                Fake { dir }
            }

            fn run(&self, args: &[&str]) -> u8 {
                run(&self.dir.join("docker-compose"), &owned(args))
            }

            fn calls(&self) -> Vec<String> {
                std::fs::read_to_string(self.dir.join("calls"))
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_owned)
                    .collect()
            }
        }

        impl Drop for Fake {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.dir);
            }
        }

        fn config(extra: &str) -> String {
            let image = format!("ghcr.io/logicplanes/kaeso-gateway:40.2.1@sha256:{D}");
            format!(
                r#"{{"services":{{"gateway":{{"image":"{image}"}},"updater":{{"image":"{}"}}{extra}}}}}"#,
                image.replace("gateway", "updater")
            )
        }

        #[test]
        fn each_task_runs_compose_with_explicit_services() {
            let fake = Fake::new("tasks", &config(""), 0);
            assert_eq!(fake.run(&["stop"]), DONE);
            assert_eq!(fake.run(&["pull"]), DONE);
            assert_eq!(fake.run(&["up", "600"]), DONE);
            assert_eq!(fake.run(&["replace", "120"]), DONE);
            assert_eq!(
                fake.calls(),
                [
                    "stop gateway console",
                    "pull --quiet --policy missing gateway",
                    "up --detach --wait --wait-timeout 600 gateway",
                    "up --detach --no-deps --wait --wait-timeout 120 updater",
                ]
            );
        }

        #[test]
        fn an_unpinned_image_is_refused_before_compose_touches_anything() {
            let fake = Fake::new("unpinned", &config(r#","own":{"image":"example/own"}"#), 0);
            assert_eq!(fake.run(&["pull"]), REFUSED);
            assert_eq!(fake.run(&["up", "600"]), REFUSED);
            assert!(fake.calls().is_empty());
        }

        #[test]
        fn a_failing_compose_and_wrong_arguments_fail() {
            let fake = Fake::new("failing", &config(""), 1);
            assert_eq!(fake.run(&["stop"]), FAILED);
            for wrong in [
                &[][..],
                &["up"],
                &["up", "soon"],
                &["stop", "now"],
                &["down"],
            ] {
                assert_eq!(fake.run(wrong), USAGE, "{wrong:?}");
            }
            let nothing = Fake::new("nothing", r#"{"services":{"gateway":{"image":"g"}}}"#, 0);
            assert_eq!(nothing.run(&["replace", "60"]), DONE);
            assert!(nothing.calls().is_empty());
            assert_eq!(nothing.run(&["pull"]), REFUSED);
            let broken = Fake::new("broken", "{", 0);
            assert_eq!(broken.run(&["pull"]), FAILED);
            assert_eq!(
                run(Path::new("/nonexistent/docker-compose"), &owned(&["stop"])),
                FAILED
            );
        }
    }
}
