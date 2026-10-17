use serde_json::{Map, Value, json};

pub const IMAGE_LABEL: &str = "com.docker.compose.image";
pub const VERSION_LABEL: &str = "org.opencontainers.image.version";

const SAME_AS_IMAGE: &[&str] = &[
    "Cmd",
    "Entrypoint",
    "Healthcheck",
    "WorkingDir",
    "User",
    "StopSignal",
    "Shell",
    "OnBuild",
];
const ENDPOINT_FIELDS: &[&str] = &["Aliases", "IPAMConfig", "Links", "DriverOpts"];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CloneError {
    #[error("the container has no {0}")]
    Missing(&'static str),
}

fn object<'a>(value: &'a Value, key: &'static str) -> Result<&'a Map<String, Value>, CloneError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or(CloneError::Missing(key))
}

fn without_image_entries(container: Option<&Value>, image: Option<&Value>) -> Option<Value> {
    let list = container?.as_array()?;
    let inherited: Vec<&Value> = image
        .and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    Some(Value::Array(
        list.iter()
            .filter(|v| !inherited.contains(v))
            .cloned()
            .collect(),
    ))
}

fn without_image_keys(
    container: Option<&Value>,
    image: Option<&Value>,
    same_value: bool,
) -> Option<Value> {
    let map = container?.as_object()?;
    let inherited = image.and_then(Value::as_object);
    Some(Value::Object(
        map.iter()
            .filter(|(k, v)| match inherited.and_then(|i| i.get(*k)) {
                Some(iv) if same_value => iv != *v,
                Some(_) => false,
                None => true,
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    ))
}

pub fn name(container: &Value) -> Result<String, CloneError> {
    container
        .get("Name")
        .and_then(Value::as_str)
        .map(|n| n.trim_start_matches('/').to_owned())
        .filter(|n| !n.is_empty())
        .ok_or(CloneError::Missing("Name"))
}

pub fn version_of(image: &Value) -> Option<&str> {
    image
        .pointer(&format!(
            "/Config/Labels/{}",
            VERSION_LABEL.replace('/', "~1")
        ))?
        .as_str()
}

pub fn spec(
    container: &Value,
    old_image: &Value,
    new_image: &Value,
    reference: &str,
) -> Result<Value, CloneError> {
    let mut config = object(container, "Config")?.clone();
    let host_config = object(container, "HostConfig")?.clone();
    let image_config = old_image.get("Config");
    let inherited = |key: &str| image_config.and_then(|c| c.get(key));
    config.insert("Image".into(), Value::String(reference.to_owned()));
    if let Some(env) = without_image_entries(config.get("Env"), inherited("Env")) {
        config.insert("Env".into(), env);
    }
    let mut labels = without_image_keys(config.get("Labels"), inherited("Labels"), true)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    if let Some(id) = new_image.get("Id") {
        labels.insert(IMAGE_LABEL.into(), id.clone());
    }
    config.insert("Labels".into(), Value::Object(labels));
    for key in ["ExposedPorts", "Volumes"] {
        if let Some(v) = without_image_keys(config.get(key), inherited(key), false) {
            config.insert(key.into(), v);
        }
    }
    for key in SAME_AS_IMAGE {
        if config.get(*key).is_some() && config.get(*key) == inherited(key) {
            config.remove(*key);
        }
    }
    let id = container
        .get("Id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let short = id.get(..12).unwrap_or(id);
    if config.get("Hostname").and_then(Value::as_str) == Some(short) {
        config.remove("Hostname");
    }
    let networks = container
        .pointer("/NetworkSettings/Networks")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let endpoints: Map<String, Value> = networks
        .into_iter()
        .map(|(net, settings)| {
            let mut endpoint = Map::new();
            for field in ENDPOINT_FIELDS {
                match settings.get(*field) {
                    Some(Value::Null) | None => {}
                    Some(Value::Array(aliases)) if *field == "Aliases" => {
                        let kept: Vec<Value> = aliases
                            .iter()
                            .filter(|a| a.as_str() != Some(short))
                            .cloned()
                            .collect();
                        endpoint.insert((*field).into(), Value::Array(kept));
                    }
                    Some(v) => {
                        endpoint.insert((*field).into(), v.clone());
                    }
                }
            }
            (net, Value::Object(endpoint))
        })
        .collect();
    let mut body = config;
    body.insert("HostConfig".into(), Value::Object(host_config));
    body.insert(
        "NetworkingConfig".into(),
        json!({ "EndpointsConfig": endpoints }),
    );
    Ok(Value::Object(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn container() -> Value {
        json!({
            "Id": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "Name": "/kaeso-gateway-1",
            "Config": {
                "Hostname": "0123456789ab",
                "Image": "ghcr.io/logicplanes/kaeso-gateway:0.2.0",
                "Env": ["KAESO_PUBLIC_URL=https://kaeso.example.com", "PATH=/usr/local/bin:/usr/bin", "KAESO_LISTEN=0.0.0.0:8080"],
                "Cmd": ["serve"],
                "Entrypoint": ["/usr/local/bin/kaeso"],
                "Healthcheck": { "Test": ["CMD", "/usr/local/bin/kaeso", "healthcheck"] },
                "User": "nonroot:nonroot",
                "ExposedPorts": { "8080/tcp": {}, "9100/tcp": {} },
                "Labels": {
                    "com.docker.compose.project": "kaeso",
                    "com.docker.compose.service": "gateway",
                    "com.docker.compose.image": "sha256:old",
                    "org.opencontainers.image.version": "0.2.0",
                    "org.opencontainers.image.title": "Kaeso gateway"
                }
            },
            "HostConfig": {
                "RestartPolicy": { "Name": "unless-stopped" },
                "ReadonlyRootfs": true,
                "CapDrop": ["ALL"],
                "Mounts": [{ "Type": "bind", "Source": "/opt/kaeso/master-keys", "Target": "/run/secrets/master_keys", "ReadOnly": true }]
            },
            "NetworkSettings": {
                "Networks": {
                    "kaeso_edge": { "Aliases": ["kaeso-gateway-1", "gateway", "0123456789ab"], "IPAMConfig": null, "DNSNames": ["gateway"], "MacAddress": "02:42:ac:11:00:02" },
                    "kaeso_data": { "Aliases": ["kaeso-gateway-1", "gateway"], "Links": null }
                }
            }
        })
    }

    fn old_image() -> Value {
        json!({
            "Id": "sha256:old",
            "Config": {
                "Env": ["PATH=/usr/local/bin:/usr/bin", "KAESO_LISTEN=0.0.0.0:8080"],
                "Cmd": ["serve"],
                "Entrypoint": ["/usr/local/bin/kaeso"],
                "Healthcheck": { "Test": ["CMD", "/usr/local/bin/kaeso", "healthcheck"] },
                "User": "nonroot:nonroot",
                "ExposedPorts": { "8080/tcp": {}, "9100/tcp": {} },
                "Labels": { "org.opencontainers.image.version": "0.2.0", "org.opencontainers.image.title": "Kaeso gateway" }
            }
        })
    }

    #[test]
    fn the_clone_keeps_the_service_and_drops_what_came_from_the_old_image() {
        let spec = spec(
            &container(),
            &old_image(),
            &json!({ "Id": "sha256:new" }),
            "ghcr.io/logicplanes/kaeso-gateway@sha256:abc",
        )
        .unwrap();
        assert_eq!(
            spec["Image"],
            "ghcr.io/logicplanes/kaeso-gateway@sha256:abc"
        );
        assert_eq!(
            spec["Env"],
            json!(["KAESO_PUBLIC_URL=https://kaeso.example.com"])
        );
        assert_eq!(
            spec["Labels"],
            json!({
                "com.docker.compose.project": "kaeso",
                "com.docker.compose.service": "gateway",
                "com.docker.compose.image": "sha256:new",
            })
        );
        for gone in ["Cmd", "Entrypoint", "Healthcheck", "User", "Hostname"] {
            assert!(spec.get(gone).is_none(), "{gone}");
        }
        assert_eq!(spec["ExposedPorts"], json!({}));
        assert_eq!(spec["HostConfig"], container()["HostConfig"]);
        assert_eq!(
            spec["NetworkingConfig"]["EndpointsConfig"],
            json!({
                "kaeso_edge": { "Aliases": ["kaeso-gateway-1", "gateway"] },
                "kaeso_data": { "Aliases": ["kaeso-gateway-1", "gateway"] },
            })
        );
    }

    #[test]
    fn settings_of_the_service_that_differ_from_the_image_stay() {
        let mut c = container();
        c["Config"]["Cmd"] = json!(["serve", "--verbose"]);
        c["Config"]["Hostname"] = json!("gateway-host");
        c["Config"]["Labels"]["org.opencontainers.image.title"] = json!("Our gateway");
        let spec = spec(&c, &old_image(), &json!({}), "x").unwrap();
        assert_eq!(spec["Cmd"], json!(["serve", "--verbose"]));
        assert_eq!(spec["Hostname"], "gateway-host");
        assert_eq!(
            spec["Labels"]["org.opencontainers.image.title"],
            "Our gateway"
        );
        assert_eq!(spec["Labels"]["com.docker.compose.image"], "sha256:old");
    }

    #[test]
    fn a_container_without_its_configuration_is_refused() {
        assert_eq!(
            spec(&json!({}), &old_image(), &json!({}), "x"),
            Err(CloneError::Missing("Config"))
        );
        let mut c = container();
        c.as_object_mut().unwrap().remove("HostConfig");
        assert_eq!(
            spec(&c, &old_image(), &json!({}), "x"),
            Err(CloneError::Missing("HostConfig"))
        );
    }

    #[test]
    fn names_and_versions_are_read_from_docker() {
        assert_eq!(name(&container()).unwrap(), "kaeso-gateway-1");
        assert_eq!(
            name(&json!({ "Name": "/" })),
            Err(CloneError::Missing("Name"))
        );
        assert_eq!(version_of(&old_image()), Some("0.2.0"));
        assert_eq!(version_of(&json!({})), None);
    }
}
