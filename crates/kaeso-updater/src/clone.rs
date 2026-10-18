use serde_json::{Map, Value, json};

pub const IMAGE_LABEL: &str = "com.docker.compose.image";
pub const VERSION_LABEL: &str = "org.opencontainers.image.version";
pub const START_PERIOD_LABEL: &str = "app.kaeso.updater.start-period-was";
const NO_START_PERIOD: &str = "none";

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

pub fn start_period(spec: &mut Value, period: std::time::Duration) {
    let nanos = u64::try_from(period.as_nanos()).unwrap_or(u64::MAX);
    let Some(body) = spec.as_object_mut() else {
        return;
    };
    let check = body
        .entry("Healthcheck")
        .or_insert_with(|| Value::Object(Map::new()));
    if !check.is_object() {
        *check = Value::Object(Map::new());
    }
    let mut was = None;
    if let Some(check) = check.as_object_mut() {
        was = check.get("StartPeriod").and_then(Value::as_u64);
        check.insert("StartPeriod".into(), json!(was.unwrap_or(0).max(nanos)));
    }
    let marker = was.map_or_else(|| NO_START_PERIOD.to_owned(), |n| n.to_string());
    let labels = body
        .entry("Labels")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(labels) = labels.as_object_mut() {
        labels.insert(START_PERIOD_LABEL.into(), Value::String(marker));
    }
}

fn without_added_start_period(config: &mut Map<String, Value>) {
    let was = config
        .get_mut("Labels")
        .and_then(Value::as_object_mut)
        .and_then(|labels| labels.remove(START_PERIOD_LABEL));
    let Some(was) = was.as_ref().and_then(Value::as_str) else {
        return;
    };
    let Some(check) = config.get_mut("Healthcheck").and_then(Value::as_object_mut) else {
        return;
    };
    match was.parse::<u64>() {
        Ok(nanos) => {
            check.insert("StartPeriod".into(), json!(nanos));
        }
        Err(_) => {
            check.remove("StartPeriod");
        }
    }
    if check.is_empty() {
        config.remove("Healthcheck");
    }
}

pub fn spec(
    container: &Value,
    old_image: &Value,
    new_image: &Value,
    reference: &str,
) -> Result<Value, CloneError> {
    let mut config = object(container, "Config")?.clone();
    without_added_start_period(&mut config);
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
                "Image": "ghcr.io/logicplanes/kaeso-gateway:40.2.0",
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
                    "org.opencontainers.image.version": "40.2.0",
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
                "Labels": { "org.opencontainers.image.version": "40.2.0", "org.opencontainers.image.title": "Kaeso gateway" }
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
    fn the_start_period_covers_the_migrations_and_keeps_the_rest_of_the_check() {
        let ten_minutes = std::time::Duration::from_secs(600);
        let mut inherited = spec(&container(), &old_image(), &json!({}), "x").unwrap();
        start_period(&mut inherited, ten_minutes);
        assert_eq!(
            inherited["Healthcheck"],
            json!({ "StartPeriod": 600_000_000_000u64 })
        );
        let mut own = json!({ "Healthcheck": { "Test": ["CMD", "probe"], "StartPeriod": 900_000_000_000u64 } });
        start_period(&mut own, ten_minutes);
        assert_eq!(
            own["Healthcheck"],
            json!({ "Test": ["CMD", "probe"], "StartPeriod": 900_000_000_000u64 })
        );
        let mut odd = json!({ "Healthcheck": null });
        start_period(&mut odd, ten_minutes);
        assert_eq!(odd["Healthcheck"]["StartPeriod"], 600_000_000_000u64);
        assert_eq!(inherited["Labels"][START_PERIOD_LABEL], "none");
        assert_eq!(own["Labels"][START_PERIOD_LABEL], "900000000000");
    }

    #[test]
    fn the_next_update_takes_the_new_images_health_check_again() {
        let ten_minutes = std::time::Duration::from_secs(600);
        let mut updated = container();
        let mut merged = old_image()["Config"]["Healthcheck"].clone();
        merged["StartPeriod"] = json!(600_000_000_000u64);
        updated["Config"]["Healthcheck"] = merged;
        updated["Config"]["Labels"][START_PERIOD_LABEL] = json!("none");
        let next = spec(
            &updated,
            &old_image(),
            &json!({ "Id": "sha256:newer" }),
            "x",
        )
        .unwrap();
        assert!(
            next.get("Healthcheck").is_none(),
            "the old image's check is not carried over: {next}"
        );
        assert!(next["Labels"].get(START_PERIOD_LABEL).is_none());
        let mut next = next;
        start_period(&mut next, ten_minutes);
        assert_eq!(
            next["Healthcheck"],
            json!({ "StartPeriod": 600_000_000_000u64 })
        );

        let mut own = container();
        own["Config"]["Healthcheck"] =
            json!({ "Test": ["CMD", "probe"], "StartPeriod": 600_000_000_000u64 });
        own["Config"]["Labels"][START_PERIOD_LABEL] = json!("30000000000");
        let kept = spec(&own, &old_image(), &json!({}), "x").unwrap();
        assert_eq!(
            kept["Healthcheck"],
            json!({ "Test": ["CMD", "probe"], "StartPeriod": 30_000_000_000u64 }),
            "a check of the service's own keeps its own start period"
        );

        let mut bare = container();
        bare["Config"]["Healthcheck"] = json!({ "StartPeriod": 600_000_000_000u64 });
        bare["Config"]["Labels"][START_PERIOD_LABEL] = json!("none");
        assert!(
            spec(&bare, &old_image(), &json!({}), "x")
                .unwrap()
                .get("Healthcheck")
                .is_none()
        );
    }

    #[test]
    fn names_and_versions_are_read_from_docker() {
        assert_eq!(name(&container()).unwrap(), "kaeso-gateway-1");
        assert_eq!(
            name(&json!({ "Name": "/" })),
            Err(CloneError::Missing("Name"))
        );
        assert_eq!(version_of(&old_image()), Some("40.2.0"));
        assert_eq!(version_of(&json!({})), None);
    }
}
