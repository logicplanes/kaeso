import json
import os
import re
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
LANGUAGES = ("en", "de", "zh-CN")
PAGES = sorted(
    [ROOT / "README.md", *(ROOT / "apps" / "docs" / "content").rglob("*.md")],
)
YAML_BLOCK = re.compile(r"```yaml\n(.*?)```", re.S)


def config(*overrides: Path) -> dict:
    env = {k: v for k, v in os.environ.items() if not k.startswith("COMPOSE_")}
    passwords = ("POSTGRES_PASSWORD", "POSTGRES_SUPERUSER_PASSWORD", "CLICKHOUSE_PASSWORD", "VALKEY_PASSWORD")
    env |= {name: "pinned" for name in passwords}
    files = ["-f", "deploy/compose.yaml", *(arg for o in overrides for arg in ("-f", str(o)))]
    out = subprocess.run(
        ["docker", "compose", *files, "--env-file", "deploy/.env.example", "config", "--format", "json"],
        cwd=ROOT,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return json.loads(out)


def test_every_compose_up_in_the_docs_waits_for_the_services():
    found = []
    for page in PAGES:
        for line in page.read_text().splitlines():
            for command in re.findall(r"docker compose(?: -f \S+)* up -d[^`|;]*", line):
                found.append(command)
                assert "--wait" in command, f"{page.relative_to(ROOT)}: {command}"
    assert len(found) >= 3 * 8


def test_every_service_has_a_health_check_for_wait_to_wait_for():
    services = config()["services"]
    own = {name for name, service in services.items() if service.get("healthcheck", {}).get("test")}
    images = {
        name
        for name in ("gateway", "console")
        if re.search(r"^HEALTHCHECK .+ CMD ", (ROOT / "deploy" / f"{name}.Dockerfile").read_text(), re.M)
    }
    assert own | images == set(services)
    assert not own & images


def test_caddy_reports_healthy_from_a_site_of_its_own_on_the_loopback_address():
    caddy = config()["services"]["caddy"]
    check = caddy["healthcheck"]["test"]
    assert check[-1] == "http://127.0.0.1:2080/"
    assert all(p["target"] != 2080 for p in caddy["ports"])
    caddyfile = config()["configs"]["caddyfile"]["content"]
    assert "http://127.0.0.1:2080 {\n\tbind 127.0.0.1\n" in caddyfile


@pytest.mark.parametrize("language", LANGUAGES)
def test_the_reverse_proxy_overrides_in_the_docs_merge_with_the_compose_file(language, tmp_path):
    page = ROOT / "apps" / "docs" / "content" / language / "self-hosting" / "domains-and-tls.md"
    loopback, network = YAML_BLOCK.findall(page.read_text())
    (tmp_path / "loopback.yaml").write_text(loopback)
    (tmp_path / "network.yaml").write_text(network)

    published = config(tmp_path / "loopback.yaml")["services"]
    assert "caddy" not in published
    assert [p["published"] for p in published["gateway"]["ports"]] == ["8080"]
    assert [p["host_ip"] for p in published["console"]["ports"]] == ["127.0.0.1"]

    joined = config(tmp_path / "network.yaml")
    assert "caddy" not in joined["services"]
    assert joined["networks"]["kaeso-proxy"]["external"] is True
    for service, alias in (("gateway", "kaeso-gateway"), ("console", "kaeso-console")):
        networks = joined["services"][service]["networks"]
        assert networks["kaeso-proxy"]["aliases"] == [alias]
        assert "edge" in networks
    assert set(joined["services"]["gateway"]["networks"]) == {"edge", "data", "kaeso-proxy"}
