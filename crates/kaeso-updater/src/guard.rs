use kaeso_update::manifest::{Image, Service, is_digest};
use serde_json::Value;

pub const CONFINED: &[&str] = &[
    "Privileged",
    "CapAdd",
    "CapDrop",
    "Devices",
    "DeviceRequests",
    "DeviceCgroupRules",
    "PidMode",
    "IpcMode",
    "UsernsMode",
    "UTSMode",
    "CgroupnsMode",
    "NetworkMode",
    "Binds",
    "Mounts",
    "VolumesFrom",
    "SecurityOpt",
    "Sysctls",
    "Runtime",
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Refused {
    #[error("`{0}` is not a Kaeso image by digest")]
    Image(String),
    #[error("the new container would be named {found}, not {expected}")]
    Name { expected: String, found: String },
    #[error("the new container's HostConfig.{0} differs from the container it replaces")]
    HostConfig(&'static str),
    #[error("the container to replace has no HostConfig")]
    NoHostConfig,
}

pub fn image(image: &Image) -> Result<(), Refused> {
    let known = Service::ALL.iter().any(|s| s.image() == image.name);
    if known && is_digest(&image.digest) {
        Ok(())
    } else {
        Err(Refused::Image(image.reference()))
    }
}

fn reference(spec: &Value) -> Result<(), Refused> {
    let text = spec
        .get("Image")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let parsed = text.split_once('@').map(|(name, digest)| Image {
        name: name.to_owned(),
        digest: digest.to_owned(),
    });
    match parsed {
        Some(i) => image(&i),
        None => Err(Refused::Image(text.to_owned())),
    }
}

fn unset(value: Option<&Value>) -> Option<&Value> {
    match value {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) if a.is_empty() => None,
        Some(Value::Object(o)) if o.is_empty() => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(Value::Bool(false)) => None,
        Some(v) => Some(v),
    }
}

pub fn container(name: &str, spec: &Value, replaced: &Value) -> Result<(), Refused> {
    reference(spec)?;
    let expected = crate::clone::name(replaced).map_err(|_| Refused::Name {
        expected: String::new(),
        found: name.to_owned(),
    })?;
    if name != expected {
        return Err(Refused::Name {
            expected,
            found: name.to_owned(),
        });
    }
    let old = replaced
        .get("HostConfig")
        .and_then(Value::as_object)
        .ok_or(Refused::NoHostConfig)?;
    let new = spec.get("HostConfig").and_then(Value::as_object);
    for field in CONFINED {
        if unset(new.and_then(|h| h.get(*field))) != unset(old.get(*field)) {
            return Err(Refused::HostConfig(field));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const D: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn replaced() -> Value {
        json!({
            "Name": "/kaeso-gateway-1",
            "HostConfig": {
                "Binds": null,
                "CapAdd": null,
                "CapDrop": ["ALL"],
                "NetworkMode": "kaeso_edge",
                "Privileged": false,
                "SecurityOpt": ["no-new-privileges:true"],
                "Mounts": [{ "Type": "bind", "Source": "/srv/kaeso/master-keys", "Target": "/run/secrets/master_keys" }],
                "Memory": 2_147_483_648u64
            }
        })
    }

    fn spec(host: Value) -> Value {
        json!({ "Image": format!("ghcr.io/logicplanes/kaeso-gateway@{D}"), "HostConfig": host })
    }

    #[test]
    fn a_clone_of_the_replaced_container_passes() {
        let host = replaced()["HostConfig"].clone();
        assert_eq!(
            container("kaeso-gateway-1", &spec(host), &replaced()),
            Ok(())
        );
    }

    #[test]
    fn empty_and_missing_settings_are_the_same() {
        let mut host = replaced()["HostConfig"].clone();
        host["Binds"] = json!([]);
        host["Privileged"] = Value::Null;
        host.as_object_mut().unwrap().remove("CapAdd");
        host["Memory"] = json!(1);
        assert_eq!(
            container("kaeso-gateway-1", &spec(host), &replaced()),
            Ok(())
        );
    }

    #[test]
    fn any_setting_that_widens_the_container_is_refused() {
        let wider = [
            ("Privileged", json!(true)),
            ("CapAdd", json!(["SYS_ADMIN"])),
            ("CapDrop", json!([])),
            ("Devices", json!([{ "PathOnHost": "/dev/sda" }])),
            ("PidMode", json!("host")),
            ("NetworkMode", json!("host")),
            ("Binds", json!(["/:/host"])),
            (
                "Mounts",
                json!([{ "Type": "bind", "Source": "/", "Target": "/host" }]),
            ),
            ("SecurityOpt", json!(["seccomp=unconfined"])),
            ("UsernsMode", json!("host")),
        ];
        for (field, value) in wider {
            let mut host = replaced()["HostConfig"].clone();
            host[field] = value;
            assert_eq!(
                container("kaeso-gateway-1", &spec(host), &replaced()),
                Err(Refused::HostConfig(
                    CONFINED.iter().find(|f| **f == field).unwrap()
                )),
                "{field}"
            );
        }
    }

    #[test]
    fn only_kaeso_images_by_digest_and_the_same_name_pass() {
        let host = replaced()["HostConfig"].clone();
        for image in [
            "ghcr.io/logicplanes/kaeso-gateway:0.1.0".to_owned(),
            format!("docker.io/library/alpine@{D}"),
            format!("ghcr.io/logicplanes/kaeso-gateway@{}", D.to_uppercase()),
            String::new(),
        ] {
            let mut s = spec(host.clone());
            s["Image"] = json!(image);
            assert!(
                matches!(
                    container("kaeso-gateway-1", &s, &replaced()),
                    Err(Refused::Image(_))
                ),
                "{image}"
            );
        }
        assert!(matches!(
            container("kaeso-postgres-1", &spec(host.clone()), &replaced()),
            Err(Refused::Name { .. })
        ));
        assert_eq!(
            container(
                "kaeso-gateway-1",
                &spec(host),
                &json!({ "Name": "/kaeso-gateway-1" })
            ),
            Err(Refused::NoHostConfig)
        );
    }

    #[test]
    fn only_kaeso_images_are_pulled_and_only_by_digest() {
        let ok = Image {
            name: Service::Console.image().into(),
            digest: D.into(),
        };
        assert_eq!(image(&ok), Ok(()));
        let tag = Image {
            name: Service::Console.image().into(),
            digest: "latest".into(),
        };
        assert!(image(&tag).is_err());
        let other = Image {
            name: "ghcr.io/example/miner".into(),
            digest: D.into(),
        };
        assert!(image(&other).is_err());
    }
}
