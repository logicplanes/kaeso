"""The Helm chart as `helm template` renders it, checked against the Compose install it mirrors."""

import base64
import json
import re
import shutil
import subprocess
import tomllib
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parents[2]
CHART = ROOT / "deploy" / "helm" / "kaeso"
VERSION = json.loads((ROOT / "package.json").read_text())["version"]
URL = "https://kaeso.example.com"
STORES = ["--set", "postgres.existingSecret=pg", "--set", "valkey.existingSecret=vk", "--set", "clickhouse.existingSecret=ch"]
SECRETS = {"KAESO_MASTER_KEYS", "KAESO_POSTGRES_URL", "KAESO_CLICKHOUSE_URL", "KAESO_VALKEY_URL", "KAESO_SMTP_PASSWORD"}

pytestmark = pytest.mark.skipif(shutil.which("helm") is None, reason="needs helm")


def helm(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(["helm", *args], capture_output=True, text=True, check=check)


def render(*args: str) -> list[dict]:
    out = helm("template", "kaeso", str(CHART), "--namespace", "kaeso", "--set", f"publicUrl={URL}", *args).stdout
    return [d for d in yaml.safe_load_all(out) if d]


def refused(*args: str) -> str:
    r = helm("template", "kaeso", str(CHART), *args, check=False)
    assert r.returncode != 0, r.stdout
    return r.stderr


def one(docs: list[dict], kind: str, name: str) -> dict:
    [found] = [d for d in docs if d["kind"] == kind and d["metadata"]["name"] == name]
    return found


def env_of(container: dict) -> dict[str, dict]:
    return {e["name"]: e for e in container["env"]}


@pytest.fixture(scope="module")
def default() -> list[dict]:
    return render(*STORES)


@pytest.fixture(scope="module")
def bundled() -> list[dict]:
    return render("--set", "bundled.enabled=true")


def test_the_chart_and_its_images_follow_the_release():
    chart = yaml.safe_load((CHART / "Chart.yaml").read_text())
    assert (chart["version"], chart["appVersion"]) == (VERSION, VERSION)


def test_the_bundled_stores_pin_the_images_compose_does():
    compose = yaml.safe_load((ROOT / "deploy" / "compose.yaml").read_text())["services"]
    values = yaml.safe_load((CHART / "values.yaml").read_text())["bundled"]
    for store in ["postgres", "clickhouse", "valkey"]:
        assert values[store]["image"] == compose[store]["image"], store


def test_the_gateway_runs_this_release_hardened_as_compose_runs_it(default):
    deployment = one(default, "Deployment", "kaeso-gateway")
    pod = deployment["spec"]["template"]["spec"]
    [gateway] = pod["containers"]
    assert gateway["image"] == f"ghcr.io/logicplanes/kaeso-gateway:{VERSION}"
    assert pod["terminationGracePeriodSeconds"] == 75, "the gateway's own grace and its last writes"
    # The pause, the gateway's 30 seconds for streams and up to 30 more for its last writes.
    assert gateway["lifecycle"]["preStop"]["sleep"]["seconds"] + 30 + 30 <= pod["terminationGracePeriodSeconds"]
    assert pod["securityContext"]["runAsNonRoot"] and pod["securityContext"]["runAsUser"] == 65532
    assert gateway["securityContext"] == {
        "readOnlyRootFilesystem": True,
        "allowPrivilegeEscalation": False,
        "capabilities": {"drop": ["ALL"]},
    }
    assert gateway["readinessProbe"]["httpGet"] == {"path": "/readyz", "port": "http"}
    assert gateway["livenessProbe"]["httpGet"] == {"path": "/healthz", "port": "http"}
    assert {p["name"]: p["containerPort"] for p in gateway["ports"]} == {"http": 8080, "metrics": 9100}
    assert deployment["spec"]["strategy"]["rollingUpdate"]["maxUnavailable"] == 0
    assert not pod["automountServiceAccountToken"]


def test_the_gateway_reads_secrets_only_from_secrets_and_knows_it_runs_on_helm(default):
    [gateway] = one(default, "Deployment", "kaeso-gateway")["spec"]["template"]["spec"]["containers"]
    env = env_of(gateway)
    assert env["KAESO_UPDATES_INSTALLED_WITH"]["value"] == "helm"
    assert env["KAESO_PUBLIC_URL"]["value"] == URL
    assert env["KAESO_METRICS_LISTEN"]["value"] == "0.0.0.0:9100"
    for name in SECRETS & env.keys():
        assert "value" not in env[name], f"{name} is a literal"
    assert env["KAESO_POSTGRES_URL"]["valueFrom"]["secretKeyRef"] == {"name": "pg", "key": "url"}
    mount = next(m for m in gateway["volumeMounts"] if m["name"] == "master-keys")
    assert env["KAESO_MASTER_KEYS_FILE"]["value"] == f"{mount['mountPath']}/master-keys"
    assert "KAESO_MASTER_KEYS" not in env, "as a file, never an environment variable"


def test_without_clickhouse_the_gateway_keeps_no_request_logs():
    docs = render("--set", "postgres.existingSecret=pg", "--set", "valkey.existingSecret=vk")
    [gateway] = one(docs, "Deployment", "kaeso-gateway")["spec"]["template"]["spec"]["containers"]
    assert "KAESO_CLICKHOUSE_URL" not in env_of(gateway)


def caddy_api_paths() -> list[str]:
    compose = yaml.safe_load((ROOT / "deploy" / "compose.yaml").read_text())
    caddyfile = compose["configs"]["caddyfile"]["content"]
    [line] = [line for line in caddyfile.splitlines() if line.strip().startswith("@api path ")]
    return line.split("path ", 1)[1].split()


def test_the_ingress_sends_the_paths_compose_routes_to_the_gateway_there_and_the_rest_to_the_console(default):
    ingress = one(default, "Ingress", "kaeso")
    [rule] = ingress["spec"]["rules"]
    assert rule["host"] == "kaeso.example.com"
    paths = rule["http"]["paths"]
    gateway = [p["path"] for p in paths if p["backend"]["service"]["name"] == "kaeso-gateway"]
    assert all(p["pathType"] == "Prefix" for p in paths)
    # Caddy names /mcp and /mcp/* apart; a Kubernetes prefix covers both.
    expected = sorted({p.removesuffix("/*") for p in caddy_api_paths()})
    assert sorted(gateway) == expected
    assert paths[-1]["path"] == "/" and paths[-1]["backend"]["service"]["name"] == "kaeso-console"
    assert ingress["spec"]["tls"] == [{"hosts": ["kaeso.example.com"], "secretName": "kaeso-tls"}]


def test_an_nginx_ingress_lets_streams_stay_silent_and_large_bodies_through():
    docs = render(*STORES, "--set", "ingress.className=nginx", "--set", "ingress.tls.certManager.issuer=letsencrypt")
    annotations = one(docs, "Ingress", "kaeso")["metadata"]["annotations"]
    assert annotations["nginx.ingress.kubernetes.io/proxy-read-timeout"] == "3600"
    assert annotations["nginx.ingress.kubernetes.io/proxy-body-size"] == "64m"
    assert annotations["nginx.ingress.kubernetes.io/proxy-buffering"] == "off"
    assert annotations["cert-manager.io/cluster-issuer"] == "letsencrypt"
    plain = one(render(*STORES), "Ingress", "kaeso")["metadata"]["annotations"]
    assert not any(k.startswith("nginx.") for k in plain or {})


def test_an_http_route_sends_the_same_paths(default):
    docs = render(*STORES, "--set", "httpRoute.enabled=true", "--set", "httpRoute.parentRefs[0].name=public", "--set", "ingress.enabled=false")
    route = one(docs, "HTTPRoute", "kaeso")
    assert route["spec"]["hostnames"] == ["kaeso.example.com"]
    gateway, console = route["spec"]["rules"]
    ingress = one(default, "Ingress", "kaeso")["spec"]["rules"][0]["http"]["paths"]
    assert [m["path"]["value"] for m in gateway["matches"]] == [p["path"] for p in ingress[:-1]]
    assert gateway["backendRefs"] == [{"name": "kaeso-gateway", "port": 8080}]
    assert console == {"matches": [{"path": {"type": "PathPrefix", "value": "/"}}], "backendRefs": [{"name": "kaeso-console", "port": 3000}]}
    assert not [d for d in docs if d["kind"] == "Ingress"]
    assert "set httpRoute.parentRefs" in refused("--set", f"publicUrl={URL}", *STORES, "--set", "httpRoute.enabled=true")


def test_migrations_run_once_before_the_gateways_change_with_the_gateways_settings(default, bundled):
    job = one(default, "Job", "kaeso-migrate")
    assert job["metadata"]["annotations"]["helm.sh/hook"] == "pre-install,pre-upgrade"
    [migrate] = job["spec"]["template"]["spec"]["containers"]
    [gateway] = one(default, "Deployment", "kaeso-gateway")["spec"]["template"]["spec"]["containers"]
    assert migrate["args"] == ["migrate"] and migrate["image"] == gateway["image"]
    assert migrate["env"] == gateway["env"]
    assert migrate["securityContext"] == gateway["securityContext"]
    # The bundled stores start with the release, so the first migration waits for them.
    assert one(bundled, "Job", "kaeso-migrate")["metadata"]["annotations"]["helm.sh/hook"] == "post-install,pre-upgrade"
    assert not [d for d in render(*STORES, "--set", "migrations.enabled=false") if d["kind"] == "Job"]


def test_the_chart_makes_master_keys_the_gateway_reads_and_keeps_them(default):
    secret = one(default, "Secret", "kaeso-generated")
    annotations = secret["metadata"]["annotations"]
    assert annotations["helm.sh/hook"] == "pre-install,pre-upgrade"
    assert annotations["helm.sh/resource-policy"] == "keep"
    key = secret["stringData"]["master-keys"]
    match = re.fullmatch(r"(k[a-z0-9]{10}):([A-Za-z0-9+/=]+)", key)
    assert match and len(base64.b64decode(match.group(2))) == 32, key
    assert "postgres-url" not in secret["stringData"]
    own = render(*STORES, "--set", "masterKeys.existingSecret=keys", "--set", "masterKeys.key=ring")
    assert not [d for d in own if d["kind"] == "Secret"]
    [volume] = [v for v in one(own, "Deployment", "kaeso-gateway")["spec"]["template"]["spec"]["volumes"] if v["name"] == "master-keys"]
    assert volume["secret"]["secretName"] == "keys" and volume["secret"]["items"] == [{"key": "ring", "path": "master-keys"}]


def test_the_bundled_stores_use_the_generated_credentials_and_compose_settings(bundled):
    data = one(bundled, "Secret", "kaeso-generated")["stringData"]
    assert data["postgres-url"] == f"postgres://kaeso:{data['postgres-password']}@kaeso-postgres:5432/kaeso"
    assert data["clickhouse-url"] == f"http://kaeso:{data['clickhouse-password']}@kaeso-clickhouse:8123"
    assert data["valkey-url"] == f"redis://:{data['valkey-password']}@kaeso-valkey:6379"
    assert data["postgres-password"] != data["postgres-superuser-password"]
    [gateway] = one(bundled, "Deployment", "kaeso-gateway")["spec"]["template"]["spec"]["containers"]
    assert env_of(gateway)["KAESO_VALKEY_URL"]["valueFrom"]["secretKeyRef"] == {"name": "kaeso-generated", "key": "valkey-url"}
    for store in ["postgres", "clickhouse", "valkey"]:
        assert one(bundled, "StatefulSet", f"kaeso-{store}")["spec"]["replicas"] == 1
        assert one(bundled, "Service", f"kaeso-{store}")
    valkey = one(bundled, "StatefulSet", "kaeso-valkey")["spec"]["template"]["spec"]["containers"][0]
    assert valkey["args"][valkey["args"].index("--maxmemory-policy") + 1] == "noeviction"
    compose = yaml.safe_load((ROOT / "deploy" / "compose.yaml").read_text())["configs"]
    xml = one(bundled, "ConfigMap", "kaeso-bundled")["data"]["clickhouse.xml"]
    removed = re.findall(r"<(\w+) remove=\"1\"/>", compose["clickhouse"]["content"])
    assert removed and re.findall(r"<(\w+) remove=\"1\"/>", xml) == removed


def test_settings_render_to_the_configuration_file_the_gateway_reads():
    config = {"upstream": {"request_timeout": "900s"}, "cors": {"allowed_origins": "https://app.example.com"}, "postgres": {"pool_size": 30}}
    docs = render(*STORES, "--set-json", f"gateway.config={json.dumps(config)}")
    toml = tomllib.loads(one(docs, "ConfigMap", "kaeso-config")["data"]["kaeso.toml"])
    assert toml == config
    assert tomllib.loads(one(docs, "ConfigMap", "kaeso-migrate")["data"]["kaeso.toml"]) == config
    pod = one(docs, "Deployment", "kaeso-gateway")["spec"]["template"]
    assert env_of(pod["spec"]["containers"][0])["KAESO_CONFIG"]["value"] == "/etc/kaeso/kaeso.toml"
    assert pod["metadata"]["annotations"]["checksum/config"], "a changed file restarts the gateways"


def test_metrics_have_a_service_of_their_own_that_a_service_monitor_scrapes():
    docs = render(*STORES, "--set", "gateway.metrics.serviceMonitor.enabled=true")
    service = one(docs, "Service", "kaeso-gateway-metrics")
    monitor = one(docs, "ServiceMonitor", "kaeso-gateway")
    selected = monitor["spec"]["selector"]["matchLabels"]
    assert selected.items() <= service["metadata"]["labels"].items()
    assert monitor["spec"]["endpoints"] == [{"port": "metrics", "path": "/metrics", "interval": "30s"}]
    assert [p["port"] for p in one(docs, "Service", "kaeso-gateway")["spec"]["ports"]] == [8080]


def test_scaling_takes_an_autoscaler_and_keeps_one_pod_through_disruptions():
    docs = render(*STORES, "--set", "gateway.autoscaling.enabled=true")
    assert "replicas" not in one(docs, "Deployment", "kaeso-gateway")["spec"]
    assert one(docs, "HorizontalPodAutoscaler", "kaeso-gateway")["spec"]["minReplicas"] == 2
    assert one(docs, "PodDisruptionBudget", "kaeso-gateway")["spec"]["maxUnavailable"] == 1


def test_the_console_runs_read_only_with_caddys_scratch_space(default):
    pod = one(default, "Deployment", "kaeso-console")["spec"]["template"]["spec"]
    [console] = pod["containers"]
    assert console["image"] == f"ghcr.io/logicplanes/kaeso-console:{VERSION}"
    assert console["securityContext"]["readOnlyRootFilesystem"]
    assert sorted(m["mountPath"] for m in console["volumeMounts"]) == ["/tmp", "/var/lib/console/config", "/var/lib/console/data"]


@pytest.mark.parametrize("value", [None, "true"])
def test_search_engines_is_one_value_the_gateway_and_the_console_both_get(value):
    docs = render(*STORES, *(("--set", f"searchEngines={value}") if value else ()))
    expected = value or "false"
    for name in ("kaeso-gateway", "kaeso-console"):
        [container] = one(docs, "Deployment", name)["spec"]["template"]["spec"]["containers"]
        assert env_of(container)["KAESO_SEARCH_ENGINES"] == {"name": "KAESO_SEARCH_ENGINES", "value": expected}, name


def test_search_engines_set_for_the_gateway_alone_is_refused():
    for where in ("gateway.env.KAESO_SEARCH_ENGINES=true", "gateway.config.search_engines=true"):
        assert "set searchEngines instead: the console reads it too" in refused("--set", f"publicUrl={URL}", *STORES, "--set", where)


def test_a_digest_pins_an_image():
    digest = "sha256:" + "a" * 64
    docs = render(*STORES, "--set", f"gateway.image.digest={digest}")
    [gateway] = one(docs, "Deployment", "kaeso-gateway")["spec"]["template"]["spec"]["containers"]
    assert gateway["image"] == f"ghcr.io/logicplanes/kaeso-gateway@{digest}"


def test_the_chart_says_what_it_needs_and_refuses_values_it_doesnt_know():
    assert "set publicUrl" in refused(*STORES)
    assert "set postgres.existingSecret" in refused("--set", f"publicUrl={URL}")
    assert "replicas" in refused("--set", f"publicUrl={URL}", *STORES, "--set", "gateway.replicas=3")
    assert "transport" in refused("--set", f"publicUrl={URL}", *STORES, "--set", "mail.transport=smtps")


@pytest.mark.skipif(shutil.which("kubeconform") is None, reason="needs kubeconform")
def test_every_rendered_manifest_is_valid_kubernetes(tmp_path):
    full = [
        *STORES,
        "--set", "bundled.enabled=true",
        "--set", "gateway.metrics.serviceMonitor.enabled=true",
        "--set", "gateway.autoscaling.enabled=true",
        "--set", "httpRoute.enabled=true",
        "--set", "httpRoute.parentRefs[0].name=public",
        "--set-json", 'gateway.config={"upstream":{"request_timeout":"900s"}}',
    ]
    for name, args in [("default", STORES), ("full", full)]:
        out = helm("template", "kaeso", str(CHART), "--set", f"publicUrl={URL}", *args).stdout
        (tmp_path / f"{name}.yaml").write_text(out)
    catalog = "https://raw.githubusercontent.com/datreeio/CRDs-catalog/main/{{.Group}}/{{.ResourceKind}}_{{.ResourceAPIVersion}}.json"
    r = subprocess.run(
        ["kubeconform", "-strict", "-summary", "-kubernetes-version", "1.31.0", "-schema-location", "default", "-schema-location", catalog, str(tmp_path)],
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0, r.stdout + r.stderr


def test_helm_lint_passes():
    r = helm("lint", str(CHART), "--strict", "--set", f"publicUrl={URL}", "--set", "bundled.enabled=true", check=False)
    assert r.returncode == 0, r.stdout + r.stderr
