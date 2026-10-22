import json
import os
import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
PASSWORD = "c0ffee0ddba11"


def config(env: dict[str, str] | None = None) -> dict:
    base = {k: v for k, v in os.environ.items() if not k.startswith("COMPOSE_")}
    passwords = ("POSTGRES_PASSWORD", "POSTGRES_SUPERUSER_PASSWORD", "CLICKHOUSE_PASSWORD", "VALKEY_PASSWORD")
    base |= {name: PASSWORD for name in passwords} | (env or {})
    out = subprocess.run(
        ["docker", "compose", "-f", "deploy/compose.yaml", "--env-file", "deploy/.env.example", "--profile=updater"]
        + ["config", "--format", "json"],
        cwd=ROOT,
        env=base,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)


@pytest.fixture(scope="module")
def services() -> dict:
    return config()["services"]


HEX = "0123456789abcdef" * 4
ID = HEX
DIGEST = f"sha256:{HEX}"
KAESO = [f"ghcr.io/logicplanes/kaeso-{name}@{DIGEST}" for name in ("gateway", "console", "updater")]
UPDATER_CALLS = {
    "GET": [
        "/v1.44/version",
        "/v1.44/containers/json",
        f"/v1.44/containers/{ID[:12]}/json",
        f"/v1.44/containers/{ID}/json",
        f"/v1.44/images/{DIGEST}/json",
        *(f"/v1.44/images/{image}/json" for image in KAESO),
    ],
    "POST": [
        "/v1.44/images/create",
        *(f"/v1.44/images/{image}/tag" for image in KAESO),
        "/v1.44/containers/create",
        *(f"/v1.44/containers/{ID}/{action}" for action in ("start", "stop", "rename")),
    ],
    "DELETE": [f"/v1.44/containers/{ID}"],
}
NEVER = {
    "GET": [
        "/version",
        "/v1.43/version",
        "/v1.44/info",
        "/v1.44/images/json",
        "/v1.44/containers/kaeso-postgres-1/json",
        f"/v1.44/containers/{ID[:20]}/json",
        f"/v1.44/containers/{ID}/logs",
        f"/v1.44/containers/{ID}/archive",
        f"/v1.44/containers/{ID}/export",
        f"/v1.44/images/ghcr.io/example/kaeso-gateway@{DIGEST}/json",
        f"/v1.44/images/{DIGEST}/get",
        "/v1.44/secrets",
        "/v1.44/events",
    ],
    "POST": [
        "/v1.44/build",
        "/v1.44/images/load",
        f"/v1.44/images/{DIGEST}/tag",
        "/v1.44/images/alpine/tag",
        f"/v1.44/images/{DIGEST}/push",
        f"/v1.44/containers/{ID}/exec",
        f"/v1.44/exec/{ID}/start",
        f"/v1.44/containers/{ID}/attach",
        f"/v1.44/containers/{ID}/update",
        f"/v1.44/containers/{ID}/kill",
        f"/v1.44/containers/{ID}/commit",
        "/v1.44/containers/prune",
        "/v1.44/volumes/create",
        "/v1.44/networks/create",
        f"/v1.44/networks/{ID}/connect",
        "/v1.44/plugins/pull",
        "/v1.44/swarm/init",
        "/v1.44/services/create",
    ],
    "DELETE": [f"/v1.44/images/{DIGEST}", "/v1.44/volumes/kaeso_postgres", f"/v1.44/networks/{ID}", "/v1.44/containers/kaeso-postgres-1"],
    "PUT": [f"/v1.44/containers/{ID}/archive"],
    "HEAD": [f"/v1.44/containers/{ID}/archive"],
}


def proxy_allows(command: list[str]) -> dict[str, list[re.Pattern]]:
    allowed: dict[str, list[re.Pattern]] = {}
    for flag in command:
        found = re.fullmatch(r"-allow([A-Z]+)=(.+)", flag)
        if found:
            allowed.setdefault(found[1], []).append(re.compile(found[2]))
    return allowed


def test_the_docker_proxy_lets_the_updater_call_only_the_endpoints_it_uses():
    allowed = proxy_allows(config()["services"]["docker-proxy"]["command"])
    assert set(allowed) == {"GET", "POST", "DELETE"}
    for method, paths in UPDATER_CALLS.items():
        for path in paths:
            assert any(p.fullmatch(path) for p in allowed[method]), f"{method} {path}"
    for method, paths in NEVER.items():
        for path in paths:
            assert not any(p.fullmatch(path) for p in allowed.get(method, [])), f"{method} {path}"


def test_the_docker_proxy_allows_bind_mounts_only_from_the_install_folder(tmp_path):
    command = config({"PWD": str(tmp_path)})["services"]["docker-proxy"]["command"]
    assert f"-allowbindmountfrom={tmp_path}" in command
    assert "-allowfrom=updater" in command


def test_no_service_gains_privileges_or_keeps_capabilities_it_does_not_need(services):
    needed = {
        "caddy-init": ["CHOWN", "DAC_READ_SEARCH"],
        "caddy": [],
        "gateway": [],
        "console": [],
        "docker-proxy": [],
        "updater": [],
        "updater-init": ["CHOWN", "DAC_READ_SEARCH", "FOWNER"],
        "postgres": ["CHOWN", "DAC_OVERRIDE", "FOWNER", "SETGID", "SETUID"],
        "clickhouse": ["CHOWN", "DAC_OVERRIDE", "FOWNER", "KILL", "SETGID", "SETUID"],
        "valkey": [],
    }
    for name, service in services.items():
        assert service["security_opt"] == ["no-new-privileges:true"], name
        assert service["cap_drop"] == ["ALL"], name
        if name in needed:
            assert sorted(service.get("cap_add", [])) == needed[name], name


def test_the_edge_and_the_kaeso_images_run_without_root_on_read_only_file_systems(services):
    assert services["caddy"]["user"] == "65532:65532"
    assert services["valkey"]["user"] == "999:1000"
    for name in ("gateway", "console", "updater", "docker-proxy"):
        assert services[name]["read_only"] is True, name
    assert {t.split(":")[0] for t in services["console"]["tmpfs"]} == {"/tmp", "/var/lib/console/data", "/var/lib/console/config"}
    init = services["caddy-init"]
    assert init["network_mode"] == "none"
    assert init["command"] == ["chown", "-R", "65532:65532", "/data", "/config"]
    assert services["caddy"]["depends_on"]["caddy-init"]["condition"] == "service_completed_successfully"


def test_the_edge_runs_the_console_image_and_binds_its_ports_without_capabilities(services):
    for name in ("caddy-init", "caddy", "updater-init"):
        assert services[name]["image"] == services["console"]["image"], name
    assert not [s["image"] for s in services.values() if s["image"].startswith("caddy:")]
    edge = services["caddy"]
    assert edge["sysctls"] == {"net.ipv4.ip_unprivileged_port_start": "0"}
    assert edge["command"] == ["caddy", "run", "--config", "/etc/caddy/Caddyfile", "--adapter", "caddyfile"]
    assert {k: edge["environment"][k] for k in ("XDG_DATA_HOME", "XDG_CONFIG_HOME")} == {"XDG_DATA_HOME": "/data", "XDG_CONFIG_HOME": "/config"}


def test_the_updater_runs_as_the_user_its_files_and_volumes_belong_to(services):
    assert services["updater"]["user"] == "65532:65532"
    init = services["updater-init"]
    assert init["profiles"] == ["updater"]
    assert init["network_mode"] == "none"
    assert init["command"][-1] == (
        "chown -R 65532:65532 /maintenance /state && chgrp 65532 /kaeso/compose.yaml && chmod g=rw,o-w /kaeso/compose.yaml"
    )
    mounts = {v["target"]: v["source"] for v in init["volumes"]}
    assert {t: mounts[t] for t in ("/maintenance", "/state")} == {"/maintenance": "maintenance", "/state": "updater"}
    assert mounts["/kaeso/compose.yaml"].endswith("compose.yaml")
    assert services["updater"]["depends_on"]["updater-init"]["condition"] == "service_completed_successfully"


def test_every_service_has_memory_pids_and_log_limits_that_env_can_raise(services):
    for name, service in services.items():
        assert int(service["mem_limit"]) > 0, name
        assert service["pids_limit"] > 0, name
        assert service["logging"] == {"driver": "json-file", "options": {"max-size": "10m", "max-file": "5"}}, name
    raised = config({"GATEWAY_MEMORY": "6g", "CLICKHOUSE_PIDS": "9000"})["services"]
    assert raised["gateway"]["mem_limit"] == str(6 << 30)
    assert raised["clickhouse"]["pids_limit"] == 9000


def test_valkey_reads_its_password_from_a_file_and_never_evicts_counters():
    found = config({"VALKEY_MAXMEMORY": "300mb"})
    valkey = found["services"]["valkey"]
    assert PASSWORD not in json.dumps(valkey["command"])
    assert valkey["command"] == ["valkey-server", "/usr/local/etc/valkey/valkey.conf"]
    assert [c["target"] for c in valkey["configs"]] == ["/usr/local/etc/valkey/valkey.conf"]
    lines = found["configs"]["valkey"]["content"].splitlines()
    assert f'requirepass "{PASSWORD}"' in lines
    assert "maxmemory 300mb" in lines
    assert "maxmemory-policy noeviction" in lines
    assert "appendonly yes" in lines
