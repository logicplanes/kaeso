import json
import re
import shutil
import subprocess
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
VERSION = json.loads((ROOT / "package.json").read_text())["version"]
IMAGE = re.compile(r"ghcr\.io/logicplanes/kaeso-(gateway|console|updater):(\S+?)[\"'\s]")
API_PATHS = ["/v1/*", "/platform/*", "/mcp", "/mcp/*", "/oauth/*"]
SECRET_KEYS = {"KAESO_MASTER_KEYS", "KAESO_SMTP_PASSWORD", "KAESO_CLICKHOUSE_URL", "KAESO_POSTGRES_URL", "KAESO_VALKEY_URL"}


def images_in(text: str) -> list[tuple[str, str]]:
    return [(m.group(1), m.group(2)) for m in IMAGE.finditer(text + "\n")]


def test_every_platform_file_pins_the_gateway_and_console_to_this_release():
    for file in ["render.yaml", ".railway/railway.ts", "deploy/aws/managed.yaml", "deploy/gcp/managed/main.tf"]:
        found = images_in((ROOT / file).read_text())
        assert {name for name, _ in found} == {"gateway", "console"}, file
        assert all(tag == VERSION for _, tag in found), (file, found)


def test_the_quick_servers_install_the_newest_stable_release_unless_told_a_version():
    for file in ["deploy/aws/quick-server.yaml", "deploy/gcp/quick-server/main.tf"]:
        text = (ROOT / file).read_text()
        assert "kaeso.app/install.sh" in text and "--install-docker" in text, file
        assert not images_in(text), file
        assert VERSION not in text, file


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
    upstreams = {e["key"]: e["fromService"] for e in services["kaeso-edge"]["envVars"]}
    assert upstreams["KAESO_GATEWAY_UPSTREAM"] == {"name": "kaeso-gateway", "type": "pserv", "property": "hostport"}
    assert upstreams["KAESO_CONSOLE_UPSTREAM"] == {"name": "kaeso-console", "type": "pserv", "property": "hostport"}


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
        done = subprocess.run(["tofu", *args], cwd=cwd, capture_output=True, text=True)
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
