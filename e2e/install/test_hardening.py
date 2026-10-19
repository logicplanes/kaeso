import json
import os
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


def test_no_service_gains_privileges_or_keeps_capabilities_it_does_not_need(services):
    needed = {
        "caddy-init": ["CHOWN", "DAC_READ_SEARCH"],
        "caddy": ["NET_BIND_SERVICE"],
        "gateway": [],
        "console": [],
        "docker-proxy": [],
        "updater": [],
        "updater-init": ["CHOWN", "DAC_READ_SEARCH", "FOWNER"],
        "postgres": ["CHOWN", "DAC_OVERRIDE", "FOWNER", "SETGID", "SETUID"],
        "clickhouse": ["CHOWN", "DAC_OVERRIDE", "FOWNER", "KILL", "SETGID", "SETUID"],
        "valkey": ["CHOWN", "SETGID", "SETUID"],
    }
    for name, service in services.items():
        assert service["security_opt"] == ["no-new-privileges:true"], name
        assert service["cap_drop"] == ["ALL"], name
        if name in needed:
            assert sorted(service.get("cap_add", [])) == needed[name], name


def test_the_edge_and_the_kaeso_images_run_without_root_on_read_only_file_systems(services):
    assert services["caddy"]["user"] == "65532:65532"
    for name in ("gateway", "console", "updater", "docker-proxy"):
        assert services[name]["read_only"] is True, name
    assert {t.split(":")[0] for t in services["console"]["tmpfs"]} == {"/tmp", "/var/lib/console/data", "/var/lib/console/config"}
    init = services["caddy-init"]
    assert init["network_mode"] == "none"
    assert init["command"] == ["chown", "-R", "65532:65532", "/data", "/config"]
    assert services["caddy"]["depends_on"]["caddy-init"]["condition"] == "service_completed_successfully"


def test_the_updater_runs_as_the_user_its_files_and_volumes_belong_to(services):
    assert services["updater"]["user"] == "65532:65532"
    init = services["updater-init"]
    assert init["profiles"] == ["updater"]
    assert init["network_mode"] == "none"
    assert init["command"][-1] == (
        "chown -R 65532:65532 /maintenance /state && chgrp 65532 /kaeso/compose.yaml && chmod g+rw /kaeso/compose.yaml"
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
