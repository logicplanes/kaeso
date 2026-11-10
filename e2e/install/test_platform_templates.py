import json
import re
import shutil
import subprocess
import time
import tomllib
from pathlib import Path
from urllib.parse import quote

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
VERSION = json.loads((ROOT / "package.json").read_text())["version"]
IMAGE = re.compile(r"ghcr\.io/logicplanes/kaeso-(gateway|console|updater):(\S+?)[\"'\s]")
API_PATHS = ["/v1/*", "/v1beta/*", "/v2/*", "/platform/*", "/mcp", "/mcp/*", "/oauth/*"]
SECRET_KEYS = {"KAESO_MASTER_KEYS", "KAESO_SMTP_PASSWORD", "KAESO_CLICKHOUSE_URL", "KAESO_POSTGRES_URL", "KAESO_VALKEY_URL"}


def images_in(text: str) -> list[tuple[str, str]]:
    return [(m.group(1), m.group(2)) for m in IMAGE.finditer(text + "\n")]


def test_every_platform_file_pins_the_gateway_and_console_to_this_release():
    for file in [
        "render.yaml",
        ".railway/railway.ts",
        "deploy/aws/managed.yaml",
        "deploy/gcp/managed/main.tf",
        "deploy/coolify/compose.yaml",
        "deploy/dokploy/docker-compose.yml",
        "deploy/caprover/kaeso.yml",
    ]:
        found = images_in((ROOT / file).read_text())
        assert {name for name, _ in found} == {"gateway", "console"}, file
        assert all(tag == VERSION for _, tag in found), (file, found)


@pytest.fixture(scope="module")
def app_spec() -> dict:
    return yaml.safe_load((ROOT / ".do/app.yaml").read_text())


def test_digitalocean_runs_both_images_of_this_release_behind_the_apps_edge(app_spec):
    services = {s["name"]: s for s in app_spec["services"]}
    assert set(services) == {"kaeso-gateway", "kaeso-console"}
    for name, repository in [("kaeso-gateway", "kaeso-gateway"), ("kaeso-console", "kaeso-console")]:
        image = services[name]["image"]
        assert image == {"registry_type": "GHCR", "registry": "logicplanes", "repository": repository, "tag": VERSION}, name
    assert services["kaeso-gateway"]["http_port"] == 8080 and services["kaeso-gateway"]["health_check"]["http_path"] == "/healthz"
    assert services["kaeso-console"]["http_port"] == 3000
    rules = app_spec["ingress"]["rules"]
    gateway_prefixes = [r["match"]["path"]["prefix"] for r in rules if r["component"]["name"] == "kaeso-gateway"]
    assert gateway_prefixes == ["/v1", "/v1beta", "/v2", "/platform", "/mcp", "/oauth", "/.well-known"]
    assert all(r["component"]["preserve_path_prefix"] for r in rules if r["component"]["name"] == "kaeso-gateway")
    assert rules[-1] == {"match": {"path": {"prefix": "/"}}, "component": {"name": "kaeso-console"}}, "the console takes every other path, last"


def test_digitalocean_binds_the_databases_and_keeps_secrets_out_of_the_spec(app_spec):
    [gateway] = [s for s in app_spec["services"] if s["name"] == "kaeso-gateway"]
    env = {e["key"]: e for e in gateway["envs"]}
    for key in ["KAESO_MASTER_KEYS", "KAESO_SMTP_PASSWORD", "KAESO_CLICKHOUSE_URL"]:
        assert env[key]["type"] == "SECRET" and env[key]["value"] == "", key
    for key in ["KAESO_SETUP_EMAIL", "KAESO_SMTP_HOST", "KAESO_SMTP_USER", "KAESO_SMTP_FROM"]:
        assert env[key]["value"] == "", key
    assert env["KAESO_PUBLIC_URL"]["value"] == "${APP_URL}"
    assert env["KAESO_POSTGRES_URL"] == {"key": "KAESO_POSTGRES_URL", "type": "SECRET", "value": "${kaeso-postgres.DATABASE_URL}"}
    assert env["KAESO_VALKEY_URL"] == {"key": "KAESO_VALKEY_URL", "type": "SECRET", "value": "${kaeso-valkey.DATABASE_URL}"}
    assert env["KAESO_VALKEY_CA"]["value"] == "${kaeso-valkey.CA_CERT}", "managed Valkey speaks TLS with DigitalOcean's own authority"
    assert env["KAESO_METRICS_LISTEN"]["value"] == "127.0.0.1:9100"
    assert "fc00::/7" in env["KAESO_TRUSTED_PROXIES"]["value"]
    databases = {d["name"]: d for d in app_spec["databases"]}
    assert databases["kaeso-postgres"] == {"name": "kaeso-postgres", "engine": "PG", "version": "18", "production": False}
    assert databases["kaeso-valkey"] == {"name": "kaeso-valkey", "engine": "VALKEY", "production": True, "cluster_name": "kaeso-valkey"}


@pytest.fixture(scope="module")
def deploy_template() -> dict:
    return yaml.safe_load((ROOT / ".do/deploy.template.yaml").read_text())


def test_the_deploy_button_runs_the_app_specs_gateway_console_and_routes(app_spec, deploy_template):
    spec = deploy_template["spec"]
    services = {s["name"]: s for s in spec["services"]}
    for name, service in {s["name"]: s for s in app_spec["services"]}.items():
        if name == "kaeso-gateway":
            assert {k: v for k, v in services[name].items() if k != "envs"} == {k: v for k, v in service.items() if k != "envs"}
        else:
            assert services[name] == service, name
    assert spec["ingress"] == app_spec["ingress"]
    assert spec["region"] == app_spec["region"]


def test_the_deploy_button_keeps_valkey_inside_the_app_and_creates_only_a_dev_database(app_spec, deploy_template):
    spec = deploy_template["spec"]
    services = {s["name"]: s for s in spec["services"]}
    valkey = services["kaeso-valkey"]
    assert valkey["image"] == {"registry_type": "DOCKER_HUB", "registry": "valkey", "repository": "valkey", "tag": valkey["image"]["tag"]}
    compose = (ROOT / "deploy/compose.yaml").read_text()
    assert f"valkey/valkey:{valkey['image']['tag']}@" in compose, "the same Valkey release as the Compose package"
    assert valkey["internal_ports"] == [6379] and "http_port" not in valkey, "only the app's own components reach Valkey"
    assert "--maxmemory-policy noeviction" in valkey["run_command"]
    assert valkey["health_check"] == {"port": 6379}

    gateway = {e["key"]: e for e in services["kaeso-gateway"]["envs"]}
    [app_gateway] = [s for s in app_spec["services"] if s["name"] == "kaeso-gateway"]
    expected = {e["key"]: e for e in app_gateway["envs"] if e["key"] != "KAESO_VALKEY_CA"}
    expected["KAESO_VALKEY_URL"] = {"key": "KAESO_VALKEY_URL", "value": "redis://kaeso-valkey:6379"}
    assert gateway == expected
    assert spec["databases"] == [{"name": "kaeso-postgres", "engine": "PG", "version": "18", "production": False}]


def test_the_quick_servers_install_the_newest_stable_release_unless_told_a_version():
    for file in ["deploy/aws/quick-server.yaml", "deploy/gcp/quick-server/main.tf", "deploy/azure/quick-server.json"]:
        text = (ROOT / file).read_text()
        assert "kaeso.app/install.sh" in text and "--install-docker" in text, file
        assert not images_in(text), file
        assert VERSION not in text, file
        assert "chown" not in text, f"{file}: the installer owns the master key file; a chown after it locks the gateway out"


@pytest.fixture(scope="module")
def render() -> dict:
    return yaml.safe_load((ROOT / "render.yaml").read_text())


def test_render_runs_a_public_edge_in_front_of_private_services(render):
    services = {s["name"]: s for s in render["services"]}
    assert services["kaeso-edge"]["type"] == "web"
    assert services["kaeso-edge"]["dockerCommand"] == "kaeso-edge"
    assert services["kaeso-edge"]["healthCheckPath"] == "/kaeso-edge/healthz"
    assert services["kaeso-edge"]["image"]["url"] == services["kaeso-console"]["image"]["url"]
    for name in ["kaeso-gateway", "kaeso-console"]:
        assert services[name]["type"] == "pserv", name
    for name, service in services.items():
        if service["type"] != "keyvalue":
            assert service["autoDeployTrigger"] in ("off", False), name
            assert service["runtime"] == "image", name
    edge_env = {e["key"]: e for e in services["kaeso-edge"]["envVars"]}
    assert edge_env["KAESO_EDGE_CLIENT_IP_HEADERS"]["value"] == "CF-Connecting-IP", "Render appends to X-Forwarded-For"
    assert edge_env["KAESO_GATEWAY_UPSTREAM"]["fromService"] == {"name": "kaeso-gateway", "type": "pserv", "property": "hostport"}
    assert edge_env["KAESO_CONSOLE_UPSTREAM"]["fromService"] == {"name": "kaeso-console", "type": "pserv", "property": "hostport"}


def test_render_keeps_secrets_out_of_the_blueprint_and_counters_out_of_eviction(render):
    services = {s["name"]: s for s in render["services"]}
    gateway = {e["key"]: e for e in services["kaeso-gateway"]["envVars"]}
    for key in ["KAESO_MASTER_KEYS", "KAESO_SMTP_PASSWORD", "KAESO_CLICKHOUSE_URL", "KAESO_PUBLIC_URL", "KAESO_SETUP_EMAIL"]:
        assert gateway[key] == {"key": key, "sync": False}, key
    assert gateway["KAESO_POSTGRES_URL"]["fromDatabase"] == {"name": "kaeso-postgres", "property": "connectionString"}
    assert gateway["KAESO_VALKEY_URL"]["fromService"] == {"name": "kaeso-valkey", "type": "keyvalue", "property": "connectionString"}
    assert gateway["KAESO_METRICS_LISTEN"]["value"] == "127.0.0.1:9100"
    assert "fc00::/7" in gateway["KAESO_TRUSTED_PROXIES"]["value"]
    assert services["kaeso-valkey"] == {
        "type": "keyvalue",
        "name": "kaeso-valkey",
        "plan": services["kaeso-valkey"]["plan"],
        "region": services["kaeso-valkey"]["region"],
        "maxmemoryPolicy": "noeviction",
        "ipAllowList": [],
    }
    [database] = render["databases"]
    assert database["ipAllowList"] == [] and database["databaseName"] == "kaeso"


def test_platform_launch_links_use_the_published_templates():
    readme = (ROOT / "README.md").read_text()
    assert "https://render.com/deploy?repo=https://github.com/logicplanes/kaeso/tree/prod" in readme
    assert "https://cloud.digitalocean.com/apps/new?repo=https://github.com/logicplanes/kaeso/tree/prod&refcode=881255e24b11" in readme
    assert "https://railway.com/new/template/cmVV60?utm_medium=integration&utm_source=button&utm_campaign=kaeso" in readme
    assert "kaeso-cloudformation.s3.amazonaws.com%2Flatest%2Fquick-server.yaml" in readme
    assert "cloudshell_workspace=deploy/gcp/quick-server" in readme
    template = quote("https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/azure/quick-server.json", safe="")
    form = quote("https://raw.githubusercontent.com/logicplanes/kaeso/prod/deploy/azure/createUiDefinition.json", safe="")
    assert f"https://portal.azure.com/#create/Microsoft.Template/uri/{template}/createUIFormUri/{form}" in readme
    assert "KAESO_EDGE_CLIENT_IP_HEADERS" in (ROOT / "deploy/console/edge.Caddyfile").read_text()


def test_railway_listens_on_ipv6_and_takes_secrets_from_shared_variables():
    text = (ROOT / ".railway/railway.ts").read_text()
    assert 'import { defineRailway, image, postgres, project, redis, service } from "railway/iac"' in text
    assert 'KAESO_LISTEN: "[::]:8080"' in text, "Railway's private network is IPv6 only"
    assert 'KAESO_METRICS_LISTEN: "127.0.0.1:9100"' in text
    assert text.count('autoUpdates: { type: "disabled" }') == 3, "every image stays pinned"
    for key in ["KAESO_MASTER_KEYS", "KAESO_PUBLIC_URL", "KAESO_SMTP_PASSWORD", "KAESO_CLICKHOUSE_URL", "KAESO_SETUP_EMAIL"]:
        assert f"{key}: ctx.shared.{key}" in text, key
    assert "KAESO_POSTGRES_URL: db.env.DATABASE_URL" in text
    assert "KAESO_VALKEY_URL: cache.env.REDIS_URL" in text
    assert 'start: "kaeso-edge"' in text
    assert "${{kaeso-gateway.RAILWAY_PRIVATE_DOMAIN}}:8080" in text
    assert "${{kaeso-console.RAILWAY_PRIVATE_DOMAIN}}:3000" in text
    assert "fc00::/7" in text


def test_railway_health_checks_reach_the_port_each_service_listens_on():
    text = (ROOT / ".railway/railway.ts").read_text()
    gateway = text[text.index('service("kaeso-gateway"') : text.index('service("kaeso-console"')]
    console = text[text.index('service("kaeso-console"') : text.index('service("kaeso-edge"')]
    edge = text[text.index('service("kaeso-edge"') :]
    assert 'PORT: "8080"' in gateway and 'KAESO_LISTEN: "[::]:8080"' in gateway
    assert 'PORT: "3000"' in console, "the console listens on 3000"
    assert 'PORT: "8080"' in edge


def test_the_cloudformation_templates_lint_clean():
    done = subprocess.run(
        ["cfn-lint", "deploy/aws/quick-server.yaml", "deploy/aws/managed.yaml"], cwd=ROOT, capture_output=True, text=True
    )
    assert done.returncode == 0, done.stdout + done.stderr


@pytest.fixture(scope="module")
def managed_aws() -> dict:
    class Tagged(yaml.SafeLoader):
        pass

    def tagged(loader, tag_suffix, node):
        if isinstance(node, yaml.ScalarNode):
            return {tag_suffix: loader.construct_scalar(node)}
        if isinstance(node, yaml.SequenceNode):
            return {tag_suffix: loader.construct_sequence(node, deep=True)}
        return {tag_suffix: loader.construct_mapping(node, deep=True)}

    Tagged.add_multi_constructor("!", tagged)
    return yaml.load((ROOT / "deploy/aws/managed.yaml").read_text(), Loader=Tagged)


def test_aws_routes_api_paths_to_the_gateway_and_keeps_the_data_stores_private(managed_aws):
    resources = managed_aws["Resources"]
    rules = [r for r in resources.values() if r["Type"] == "AWS::ElasticLoadBalancingV2::ListenerRule"]
    routed = [p for r in rules for p in r["Properties"]["Conditions"][0]["PathPatternConfig"]["Values"]]
    assert set(API_PATHS + ["/.well-known/oauth-*"]) <= set(routed)
    for rule in rules:
        assert rule["Properties"]["Actions"][0]["TargetGroupArn"] == {"Ref": "GatewayTargets"}
    assert resources["HttpsListener"]["Properties"]["DefaultActions"][0]["TargetGroupArn"] == {"Ref": "ConsoleTargets"}
    assert resources["GatewayTargets"]["Properties"]["HealthCheckPath"] == "/readyz"
    idle = next(a for a in resources["LoadBalancer"]["Properties"]["LoadBalancerAttributes"] if a["Key"] == "idle_timeout.timeout_seconds")
    assert int(idle["Value"]) >= 300, "longer than the stream keepalive interval"
    assert resources["Database"]["Properties"]["PubliclyAccessible"] is False
    assert resources["Database"]["DeletionPolicy"] == "Snapshot"
    assert resources["CacheParameters"]["Properties"]["Properties"]["maxmemory-policy"] == "noeviction"
    assert resources["Cache"]["Properties"]["TransitEncryptionEnabled"] is True
    [gateway] = resources["GatewayTask"]["Properties"]["ContainerDefinitions"]
    env = {e["Name"]: e["Value"] for e in gateway["Environment"]}
    assert env["KAESO_METRICS_LISTEN"] == "127.0.0.1:9100"
    assert env["KAESO_TRUSTED_PROXIES"] == {"FindInMap": ["Network", "Vpc", "Cidr"]}, "only the VPC, where the load balancer sits"
    assert gateway["ReadonlyRootFilesystem"] is True and gateway["StopTimeout"] == 75
    assert {s["Name"] for s in gateway["Secrets"] if isinstance(s, dict) and "Name" in s} == {"KAESO_MASTER_KEYS"}
    assert managed_aws["Parameters"]["MasterKeys"]["NoEcho"] is True


@pytest.mark.parametrize("folder", ["quick-server", "managed"])
def test_the_terraform_for_gcp_is_formatted_and_valid(folder):
    assert shutil.which("tofu"), "OpenTofu is on PATH in CI and for this suite"
    cwd = ROOT / "deploy" / "gcp" / folder
    for args in [["fmt", "-check", "-diff"], ["init", "-backend=false", "-input=false", "-no-color"], ["validate", "-no-color"]]:
        # init downloads the provider from GitHub releases, which times out now and then.
        for attempt in range(4 if args[0] == "init" else 1):
            if attempt:
                time.sleep(20 * attempt)
            done = subprocess.run(["tofu", *args], cwd=cwd, capture_output=True, text=True)
            if done.returncode == 0:
                break
        assert done.returncode == 0, (folder, args, done.stdout + done.stderr)


def test_gcp_routes_api_paths_to_the_gateway_and_keeps_the_data_stores_private():
    text = (ROOT / "deploy/gcp/managed/main.tf").read_text()
    for path in API_PATHS + ["/.well-known/oauth-*"]:
        assert f'"{path}"' in text, path
    assert 'default_service = google_compute_backend_service.services["console"].id' in text
    assert 'service = google_compute_backend_service.services["gateway"].id' in text
    assert 'ingress             = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"' in text
    assert 'timeout = "3600s"' in text and "timeout_sec = 3600" in text
    assert "ipv4_enabled                                  = false" in text, "Cloud SQL has no public address"
    assert 'maxmemory-policy = "noeviction"' in text
    assert 'value = "127.0.0.1:9100"' in text
    assert "deletion_protection = true" in text
    assert 'sensitive   = true' in (ROOT / "deploy/gcp/managed/variables.tf").read_text()


def compose_service_images() -> dict[str, str]:
    services = yaml.safe_load((ROOT / "deploy/compose.yaml").read_text())["services"]
    return {name: services[name]["image"] for name in ["postgres", "clickhouse", "valkey"]}


def compose_postgres_init() -> str:
    return yaml.safe_load((ROOT / "deploy/compose.yaml").read_text())["configs"]["postgres_init"]["content"]


@pytest.mark.parametrize("file", ["deploy/coolify/compose.yaml", "deploy/dokploy/docker-compose.yml"])
def test_coolify_and_dokploy_run_the_compose_packages_services_behind_a_private_edge(file):
    services = yaml.safe_load((ROOT / file).read_text())["services"]
    assert set(services) == {"edge", "gateway", "console", "postgres", "clickhouse", "valkey"}
    assert services["edge"]["image"] == services["console"]["image"]
    assert services["edge"]["command"] == ["kaeso-edge"]
    assert "/kaeso-edge/healthz" in " ".join(services["edge"]["healthcheck"]["test"]), "the image's own check asks the console's port"
    for name, image in compose_service_images().items():
        assert services[name]["image"] == image, f"{file}: {name} runs the Compose package's release"
    for name, service in services.items():
        assert "ports" not in service, f"{file}: {name} is reached through the platform's proxy only"
        assert service["security_opt"] == ["no-new-privileges:true"], name
        assert service["cap_drop"] == ["ALL"], name
    for name in ["edge", "gateway", "console"]:
        assert services[name]["read_only"] is True, name
    assert services["valkey"]["user"] == "999:1000"
    assert "noeviction" in services["valkey"]["command"] and "--requirepass" in services["valkey"]["command"]
    init = services["postgres"]["command"][2].replace("$$", "$")
    for line in compose_postgres_init().replace("$$", "$").strip().splitlines():
        assert line in init, f"{file}: the gateway's PostgreSQL user is the Compose package's, not a superuser"
    assert services["gateway"]["stop_grace_period"] == "75s"


def environment(service: dict) -> dict[str, str]:
    env = service.get("environment", {})
    if isinstance(env, list):
        return dict(entry.split("=", 1) if "=" in entry else (entry, "") for entry in env)
    return env


def test_coolify_generates_every_secret_and_routes_its_domain_to_the_edge():
    services = yaml.safe_load((ROOT / "deploy/coolify/compose.yaml").read_text())["services"]
    assert "SERVICE_URL_EDGE_8080" in environment(services["edge"])
    gateway = environment(services["gateway"])
    assert gateway["KAESO_PUBLIC_URL"] == "${SERVICE_URL_EDGE}"
    assert gateway["KAESO_MASTER_KEYS"] == "k1:${SERVICE_REALBASE64_32_MASTERKEY}", "32 random bytes in standard base64"
    assert gateway["KAESO_METRICS_LISTEN"] == "127.0.0.1:9100"
    assert "fc00::/7" in gateway["KAESO_TRUSTED_PROXIES"]
    assert gateway["KAESO_MAIL_TRANSPORT"] == "${KAESO_MAIL_TRANSPORT:-log}", "the gateway starts without an SMTP server"
    secrets = set(re.findall(r"\$\{(SERVICE_[A-Z0-9_]+)\}", (ROOT / "deploy/coolify/compose.yaml").read_text()))
    assert secrets == {
        "SERVICE_URL_EDGE",
        "SERVICE_REALBASE64_32_MASTERKEY",
        "SERVICE_PASSWORD_POSTGRES",
        "SERVICE_PASSWORD_POSTGRESADMIN",
        "SERVICE_PASSWORD_CLICKHOUSE",
        "SERVICE_PASSWORD_VALKEY",
    }


@pytest.fixture(scope="module")
def dokploy_template() -> dict:
    return tomllib.loads((ROOT / "deploy/dokploy/template.toml").read_text())


def test_dokploy_fills_every_required_variable_from_its_template(dokploy_template):
    compose = (ROOT / "deploy/dokploy/docker-compose.yml").read_text()
    required = set(re.findall(r"\$\{([A-Z0-9_]+):\?", compose))
    env = dokploy_template["config"]["env"]
    assert required <= set(env), required - set(env)
    assert dokploy_template["config"]["domains"] == [{"serviceName": "edge", "port": 8080, "host": "${main_domain}"}]
    assert env["KAESO_PUBLIC_URL"] == "https://${main_domain}"
    assert dokploy_template["variables"]["master_key"] == "${base64:32}" and env["KAESO_MASTER_KEYS"] == "k1:${master_key}"
    for name in ["postgres_password", "postgres_superuser_password", "clickhouse_password", "valkey_password"]:
        assert dokploy_template["variables"][name] == "${hash:48}", f"{name}: Dokploy's password helper isn't cryptographically random"
    assert env["KAESO_MAIL_TRANSPORT"] == "log"


def test_the_coolify_and_dokploy_compose_files_are_valid(dokploy_template, tmp_path):
    assert shutil.which("docker"), "Docker is on PATH in CI and for this suite"
    values = {key: re.sub(r"\$\{[^}]+\}", "x", value) for key, value in dokploy_template["config"]["env"].items()}
    env_file = tmp_path / ".env"
    env_file.write_text("".join(f"{key}={value}\n" for key, value in values.items()))
    for file, args in [("deploy/dokploy/docker-compose.yml", ["--env-file", str(env_file)]), ("deploy/coolify/compose.yaml", [])]:
        done = subprocess.run(["docker", "compose", "-f", file, *args, "config", "-q"], cwd=ROOT, capture_output=True, text=True)
        assert done.returncode == 0, (file, done.stderr)


@pytest.fixture(scope="module")
def caprover() -> dict:
    return yaml.safe_load((ROOT / "deploy/caprover/kaeso.yml").read_text())


def test_caprover_uses_only_the_fields_it_reads(caprover):
    assert caprover["captainVersion"] == 4
    assert set(caprover) == {"captainVersion", "services", "caproverOneClickApp"}
    for name, service in caprover["services"].items():
        assert set(service) <= {"image", "volumes", "ports", "environment", "depends_on", "hostname", "cap_add", "command", "caproverExtra"}, name
        assert set(service.get("caproverExtra", {})) <= {"dockerfileLines", "containerHttpPort", "notExposeAsWebApp", "websocketSupport"}, name
        assert "ports" not in service, name


def test_caprover_runs_a_public_edge_that_reads_nginxs_client_address(caprover):
    services = caprover["services"]
    edge = services["$$cap_appname"]
    assert edge["image"] == services["$$cap_appname-console"]["image"]
    assert edge["command"] == ["kaeso-edge"]
    assert edge["environment"]["KAESO_EDGE_CLIENT_IP_HEADERS"] == "X-Real-IP", "CapRover's nginx appends to X-Forwarded-For"
    assert edge["caproverExtra"] == {"containerHttpPort": "8080", "websocketSupport": "true"}
    for name, service in services.items():
        if name != "$$cap_appname":
            assert service["caproverExtra"] == {"notExposeAsWebApp": "true"}, name
    gateway = services["$$cap_appname-gateway"]["environment"]
    assert gateway["KAESO_METRICS_LISTEN"] == "127.0.0.1:9100"
    assert "fc00::/7" in gateway["KAESO_TRUSTED_PROXIES"]
    for name, image in compose_service_images().items():
        assert services[f"$$cap_appname-{name}"]["image"] == image, name
    assert "noeviction" in services["$$cap_appname-valkey"]["command"]
    assert services["$$cap_appname-valkey"]["command"][0] == "docker-entrypoint.sh", "Valkey runs as its own user"
    init = services["$$cap_appname-postgres"]["command"][2]
    for line in compose_postgres_init().replace("$$", "$").strip().splitlines():
        assert line in init


def test_caprover_declares_every_variable_it_uses_and_checks_the_master_key(caprover):
    text = (ROOT / "deploy/caprover/kaeso.yml").read_text()
    used = set(re.findall(r"\$\$cap_[a-z0-9_]+", text)) - {"$$cap_appname", "$$cap_root_domain", "$$cap_gen_random_hex"}
    variables = {v["id"]: v for v in caprover["caproverOneClickApp"]["variables"]}
    assert used == set(variables)
    pattern = re.compile(variables["$$cap_master_keys"]["validRegex"].strip("/").replace("\\/", "/"))
    assert pattern.fullmatch("k0123456789:" + "A" * 43 + "=")
    assert not pattern.fullmatch("")
    assert variables["$$cap_public_url"]["defaultValue"] == "https://$$cap_appname.$$cap_root_domain"


@pytest.fixture(scope="module")
def fly() -> dict[str, dict]:
    return {name: tomllib.loads((ROOT / f"deploy/fly/{name}.toml").read_text()) for name in ["edge", "gateway", "console", "valkey"]}


def test_fly_pins_one_release_and_makes_only_the_edge_public(fly):
    images = {name: config["build"]["image"] for name, config in fly.items()}
    assert images["edge"] == images["console"] == f"ghcr.io/logicplanes/kaeso-console:{VERSION}"
    assert images["gateway"] == f"ghcr.io/logicplanes/kaeso-gateway:{VERSION}"
    assert compose_service_images()["valkey"].startswith(images["valkey"] + "@"), "the Compose package's Valkey release"
    for name in ["gateway", "console", "valkey"]:
        assert "http_service" not in fly[name] and "services" not in fly[name], f"{name} is reachable only on the private network"
        assert fly[name]["checks"], name
    edge = fly["edge"]
    assert edge["experimental"]["cmd"] == ["kaeso-edge"]
    assert edge["env"]["KAESO_EDGE_CLIENT_IP_HEADERS"] == "Fly-Client-IP", "Fly's proxy appends to X-Forwarded-For"
    assert edge["http_service"]["internal_port"] == 8080 and edge["http_service"]["force_https"] is True
    assert edge["http_service"]["checks"][0]["path"] == "/kaeso-edge/healthz"
    assert all("app" not in config for config in fly.values()), "app names are chosen with --app"


def test_fly_runs_the_gateway_on_the_ipv6_private_network_and_valkey_without_eviction(fly):
    gateway = fly["gateway"]
    assert gateway["env"]["KAESO_LISTEN"] == "[::]:8080"
    assert gateway["env"]["KAESO_METRICS_LISTEN"] == "127.0.0.1:9100"
    assert gateway["env"]["KAESO_TRUSTED_PROXIES"].startswith("fc00::/7")
    assert gateway["kill_signal"] == "SIGTERM" and gateway["kill_timeout"] == 75
    valkey = fly["valkey"]
    [command] = valkey["experimental"]["cmd"][2:]
    assert command.startswith("exec docker-entrypoint.sh valkey-server"), "Valkey runs as its own user"
    assert '--requirepass "$VALKEY_PASSWORD"' in command and "--maxmemory-policy noeviction" in command
    assert valkey["mounts"][0]["destination"] == "/data"


@pytest.fixture(scope="module")
def azure() -> dict:
    return json.loads((ROOT / "deploy/azure/quick-server.json").read_text())


def test_azure_opens_only_the_web_ports_unless_ssh_is_asked_for(azure):
    variables = azure["variables"]
    web = {(r["properties"]["protocol"], r["properties"]["destinationPortRange"]) for r in variables["webRules"]}
    assert web == {("Tcp", "80"), ("Tcp", "443"), ("Udp", "443")}
    assert all(r["properties"]["sourceAddressPrefix"] == "Internet" for r in variables["webRules"])
    [ssh] = variables["sshRule"]
    assert ssh["properties"]["destinationPortRange"] == "22"
    assert ssh["properties"]["sourceAddressPrefix"] == "[parameters('sshSourceAddress')]"
    [nsg] = [r for r in azure["resources"] if r["type"] == "Microsoft.Network/networkSecurityGroups"]
    assert nsg["properties"]["securityRules"].startswith("[if(empty(parameters('sshSourceAddress')), variables('webRules'),")
    assert azure["parameters"]["sshSourceAddress"]["defaultValue"] == ""


def test_azure_keeps_secrets_out_of_the_template_and_follows_resource_manager_practice(azure):
    parameters = azure["parameters"]
    assert parameters["adminPasswordOrKey"]["type"] == "securestring" and "defaultValue" not in parameters["adminPasswordOrKey"]
    assert parameters["location"]["defaultValue"] == "[resourceGroup().location]"
    assert all("description" in p["metadata"] for p in parameters.values())
    [vm] = [r for r in azure["resources"] if r["type"] == "Microsoft.Compute/virtualMachines"]
    assert vm["properties"]["securityProfile"]["securityType"] == "TrustedLaunch"
    image = vm["properties"]["storageProfile"]["imageReference"]
    assert image == {"publisher": "Canonical", "offer": "ubuntu-24_04-lts", "sku": "server", "version": "latest"}
    [script] = [r for r in azure["resources"] if r["type"] == "Microsoft.Compute/virtualMachines/extensions"]
    assert "--dir /opt/kaeso --install-docker --no-open" in script["properties"]["protectedSettings"]["commandToExecute"]
    assert {"url", "fqdn", "publicIp", "setupLink"} == set(azure["outputs"])
    assert "docker compose exec -T gateway kaeso setup-link" in azure["outputs"]["setupLink"]["value"]


def test_the_azure_portal_form_fills_exactly_the_templates_parameters(azure):
    form = json.loads((ROOT / "deploy/azure/createUiDefinition.json").read_text())
    assert set(form["parameters"]["outputs"]) == set(azure["parameters"])
    [server] = form["parameters"]["steps"]
    [sizes] = [e for e in server["elements"] if e["name"] == "vmSize"]
    assert sizes["constraints"]["allowedSizes"] == azure["parameters"]["vmSize"]["allowedValues"]
