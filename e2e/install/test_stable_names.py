import json
import os
import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
RELEASE_ASSET = "https://github.com/logicplanes/kaeso/releases/latest/download/compose.yaml"
VOLUMES = {
    "caddy_data": "kaeso_caddy_data",
    "caddy_config": "kaeso_caddy_config",
    "postgres": "kaeso_postgres",
    "clickhouse": "kaeso_clickhouse",
    "valkey": "kaeso_valkey",
    "maintenance": "kaeso_maintenance",
}
UPDATER_VOLUMES = VOLUMES | {"updater": "kaeso_updater"}
MOUNTS = {
    "caddy": [("caddy_data", "/data"), ("caddy_config", "/config"), ("maintenance", "/srv/maintenance")],
    "gateway": [],
    "console": [],
    "postgres": [("postgres", "/var/lib/postgresql")],
    "clickhouse": [("clickhouse", "/var/lib/clickhouse")],
    "valkey": [("valkey", "/data")],
}
UPDATER_MOUNTS = {
    "updater": [("maintenance", "/maintenance"), ("updater", "/state")],
    "docker-proxy": [],
}
IMAGES = {
    "gateway": "ghcr.io/logicplanes/kaeso-gateway",
    "console": "ghcr.io/logicplanes/kaeso-console",
    "updater": "ghcr.io/logicplanes/kaeso-updater",
}


def config(*profiles: str) -> dict:
    env = {k: v for k, v in os.environ.items() if not k.startswith("COMPOSE_")}
    passwords = ("POSTGRES_PASSWORD", "POSTGRES_SUPERUSER_PASSWORD", "CLICKHOUSE_PASSWORD", "VALKEY_PASSWORD")
    env |= {name: "pinned" for name in passwords}
    flags = [f"--profile={p}" for p in profiles]
    out = subprocess.run(
        ["docker", "compose", "-f", "deploy/compose.yaml", "--env-file", "deploy/.env.example", *flags]
        + ["config", "--format", "json"],
        cwd=ROOT,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)


@pytest.fixture(scope="module")
def compose() -> dict:
    return config()


@pytest.fixture(scope="module")
def with_updater() -> dict:
    return config("updater")


def test_the_project_services_and_volumes_keep_their_names(compose):
    assert compose["name"] == "kaeso"
    assert {key: volume["name"] for key, volume in compose["volumes"].items()} == VOLUMES
    assert set(compose["services"]) == set(MOUNTS)
    for service, mounts in MOUNTS.items():
        volumes = compose["services"][service].get("volumes", [])
        found = [(v["source"], v["target"]) for v in volumes if v["type"] == "volume"]
        assert found == mounts, service


def test_the_updater_is_opt_in_and_keeps_its_names(with_updater):
    assert {key: volume["name"] for key, volume in with_updater["volumes"].items()} == UPDATER_VOLUMES
    assert set(with_updater["services"]) == set(MOUNTS) | set(UPDATER_MOUNTS)
    for service, mounts in UPDATER_MOUNTS.items():
        assert with_updater["services"][service]["profiles"] == ["updater"]
        volumes = with_updater["services"][service].get("volumes", [])
        found = [(v["source"], v["target"]) for v in volumes if v["type"] == "volume"]
        assert found == mounts, service
    binds = [v for v in with_updater["services"]["updater"]["volumes"] if v["type"] == "bind"]
    assert [b["target"] for b in binds] == ["/kaeso/compose.yaml"]
    assert with_updater["networks"]["updates"]["internal"] is True
    assert with_updater["networks"]["docker"]["internal"] is True
    assert set(with_updater["services"]["docker-proxy"]["networks"]) == {"docker"}


def test_the_images_keep_their_repositories(with_updater):
    version = json.loads((ROOT / "package.json").read_text())["version"]
    for service, repository in IMAGES.items():
        assert with_updater["services"][service]["image"] == f"{repository}:{version}"


def test_every_release_attaches_the_compose_file_under_its_name():
    workflow = (ROOT / ".github" / "workflows" / "release.yml").read_text()
    copied = [
        line.split()[1:-1]
        for line in workflow.splitlines()
        if re.fullmatch(r'\s*cp\s+.+\s+"\$assets/"\s*', line)
    ]
    assert any("deploy/compose.yaml" in sources for sources in copied), copied
    assert any("deploy/kaeso.example.toml" in sources for sources in copied), copied
    assert re.search(r'gh release create .*"\$RUNNER_TEMP"/assets/\*', workflow)


INSTALL_PAGES = ("get-started/quick-start.md", "self-hosting/installation.md", "self-hosting/upgrades.md")


@pytest.mark.parametrize(
    "page",
    [f"apps/docs/content/{lang}/{page}" for lang in ("en", "de", "zh-CN") for page in INSTALL_PAGES],
)
def test_the_install_pages_download_the_latest_release_asset(page):
    assert f"curl -fsSLO {RELEASE_ASSET}" in (ROOT / page).read_text()
