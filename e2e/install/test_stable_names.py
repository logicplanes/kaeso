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
}
MOUNTS = {
    "caddy": [("caddy_data", "/data"), ("caddy_config", "/config")],
    "gateway": [],
    "console": [],
    "postgres": [("postgres", "/var/lib/postgresql")],
    "clickhouse": [("clickhouse", "/var/lib/clickhouse")],
    "valkey": [("valkey", "/data")],
}
IMAGES = {
    "gateway": "ghcr.io/logicplanes/kaeso-gateway",
    "console": "ghcr.io/logicplanes/kaeso-console",
}


@pytest.fixture(scope="module")
def compose() -> dict:
    env = {k: v for k, v in os.environ.items() if not k.startswith("COMPOSE_")}
    passwords = ("POSTGRES_PASSWORD", "POSTGRES_SUPERUSER_PASSWORD", "CLICKHOUSE_PASSWORD", "VALKEY_PASSWORD")
    env |= {name: "pinned" for name in passwords}
    out = subprocess.run(
        ["docker", "compose", "-f", "deploy/compose.yaml", "--env-file", "deploy/.env.example"]
        + ["config", "--format", "json"],
        cwd=ROOT,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)


def test_the_project_services_and_volumes_keep_their_names(compose):
    assert compose["name"] == "kaeso"
    assert {key: volume["name"] for key, volume in compose["volumes"].items()} == VOLUMES
    assert set(compose["services"]) == set(MOUNTS)
    for service, mounts in MOUNTS.items():
        volumes = compose["services"][service].get("volumes", [])
        found = [(v["source"], v["target"]) for v in volumes if v["type"] == "volume"]
        assert found == mounts, service


def test_the_images_keep_their_repositories(compose):
    version = json.loads((ROOT / "package.json").read_text())["version"]
    for service, repository in IMAGES.items():
        assert compose["services"][service]["image"] == f"{repository}:{version}"


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
    ["README.md", *(f"apps/docs/content/{lang}/{page}" for lang in ("en", "de", "zh-CN") for page in INSTALL_PAGES)],
)
def test_the_install_pages_download_the_latest_release_asset(page):
    assert f"curl -fsSLO {RELEASE_ASSET}" in (ROOT / page).read_text()
