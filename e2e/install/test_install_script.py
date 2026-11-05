import os
import re
import shutil
import stat
import subprocess
import sys
from pathlib import Path

import httpx
import pytest

from test_install import ROOT, built, free_port, image

PROJECT = "kaeso-install-script-test"
SCRIPT = ROOT / "deploy" / "install.sh"


@pytest.fixture(scope="module")
def installed(tmp_path_factory):
    work = tmp_path_factory.mktemp("script") / "kaeso"
    work.mkdir()
    shutil.copy(Path(__file__).with_name("compose.test.yaml"), work / "compose.test.yaml")
    port = free_port()
    env = {
        **os.environ,
        "COMPOSE_PROJECT_NAME": PROJECT,
        "COMPOSE_FILE": "compose.yaml:compose.test.yaml",
        "KAESO_COMPOSE_URL": (ROOT / "deploy" / "compose.yaml").as_uri(),
        "KAESO_TEST_PORT": str(port),
        "KAESO_TEST_GATEWAY_IMAGE": image("KAESO_TEST_GATEWAY_IMAGE", "gateway"),
        "KAESO_TEST_MOCK_IMAGE": image("KAESO_TEST_MOCK_IMAGE", "mock"),
        "KAESO_TEST_CONSOLE_IMAGE": built("KAESO_TEST_CONSOLE_IMAGE", "console"),
        "KAESO_TEST_UPDATER_IMAGE": built("KAESO_TEST_UPDATER_IMAGE", "updater"),
    }

    def install() -> subprocess.CompletedProcess:
        with SCRIPT.open() as script:
            return subprocess.run(
                ["sh", "-s", "--", "--dir", str(work), "--domain", ":80"],
                stdin=script, env=env, capture_output=True, text=True, timeout=600,
            )

    try:
        first = install()
        assert first.returncode == 0, first.stdout + first.stderr
        yield work, first, install, f"http://localhost:{port}"
    finally:
        compose = ["docker", "compose", "-p", PROJECT, "-f", "compose.yaml", "-f", "compose.test.yaml"]
        subprocess.run([*compose, "down", "-v", "--remove-orphans"], cwd=work, env=env, check=False)


def test_one_command_installs_kaeso_and_prints_the_setup_link(installed):
    work, first, _, url = installed
    assert re.search(r"/setup#token=\S+", first.stdout), first.stdout
    assert httpx.get(f"{url}/healthz", timeout=10).text == "ok"

    settings = dict(line.split("=", 1) for line in (work / ".env").read_text().splitlines())
    assert settings["KAESO_DOMAIN"] == ":80"
    assert settings["KAESO_MAIL_TRANSPORT"] == "log"
    passwords = [settings[name] for name in ("POSTGRES_PASSWORD", "POSTGRES_SUPERUSER_PASSWORD", "CLICKHOUSE_PASSWORD", "VALKEY_PASSWORD")]
    assert all(re.fullmatch(r"[0-9a-f]{48}", password) for password in passwords)
    assert len(set(passwords)) == 4

    assert stat.S_IMODE((work / ".env").stat().st_mode) == 0o600
    keys = (work / "master-keys").stat()
    if sys.platform == "darwin":
        assert (stat.S_IMODE(keys.st_mode), keys.st_uid) == (0o400, os.getuid()), "Docker Desktop and Colima read it as you"
    else:
        assert (stat.S_IMODE(keys.st_mode), keys.st_uid) == (0o440, 65532), "the gateway reads the master key as user 65532"
        assert keys.st_gid == os.getgid(), "and you can still copy it for your backups"


def test_running_it_again_keeps_the_settings_and_the_master_key(installed):
    work, _, install, url = installed
    before = {name: (work / name).read_bytes() for name in (".env", "compose.yaml")}
    again = install()
    assert again.returncode == 0, again.stdout + again.stderr
    assert {name: (work / name).read_bytes() for name in before} == before
    assert httpx.get(f"{url}/healthz", timeout=10).text == "ok"


def test_it_refuses_an_unknown_option_before_touching_anything(tmp_path):
    with SCRIPT.open() as script:
        result = subprocess.run(
            ["sh", "-s", "--", "--dir", str(tmp_path / "kaeso"), "--port", "8080"],
            stdin=script, capture_output=True, text=True,
        )
    assert result.returncode == 1
    assert "Unknown option --port" in result.stderr
    assert not (tmp_path / "kaeso").exists()


def test_it_refuses_a_second_install_in_another_folder(installed, tmp_path):
    other = tmp_path / "elsewhere"
    with SCRIPT.open() as script:
        result = subprocess.run(
            ["sh", "-s", "--", "--dir", str(other), "--domain", ":80"],
            stdin=script, env={**os.environ, "COMPOSE_PROJECT_NAME": PROJECT}, capture_output=True, text=True, timeout=120,
        )
    assert result.returncode == 1, result.stdout + result.stderr
    assert f"Kaeso is already installed on this machine, in {installed[0].resolve()}" in result.stderr
    assert not other.exists(), "it leaves no folder behind"


def run_part(call: str, tmp_path: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
    """Runs one of the installer's functions, without running the installer."""
    functions = SCRIPT.read_text().rsplit("main \"$@\"", 1)[0]
    return subprocess.run(
        ["sh", "-c", f"{functions}\n{call}"], cwd=tmp_path, env={**os.environ, **(env or {})}, capture_output=True, text=True,
    )


@pytest.mark.parametrize(
    ("typed", "domain"),
    [
        ("kaeso.example.com", "kaeso.example.com"),
        ("https://Kaeso.Example.com/", "kaeso.example.com"),
        ("http://kaeso.example.com/setup", "kaeso.example.com"),
        ("  localhost  ", "localhost"),
        ("203.0.113.7", "203.0.113.7"),
        (":80", ":80"),
        ("kaeso.example.com:8443", None),
        ("not a domain", None),
        ("-kaeso.example.com", None),
        ("kaeso..example.com", None),
        ("kaeso_example.com", None),
    ],
)
def test_a_typed_domain_is_cleaned_or_refused(typed, domain, tmp_path):
    result = run_part(f"clean_domain '{typed}'", tmp_path)
    assert (result.stdout.strip() if result.returncode == 0 else None) == domain


def stub(tmp_path: Path, name: str, body: str) -> dict[str, str]:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(exist_ok=True)
    (bin_dir / name).write_text(f"#!/bin/sh\n{body}\n")
    (bin_dir / name).chmod(0o755)
    return {"PATH": f"{bin_dir}:{os.environ['PATH']}"}


@pytest.mark.parametrize(
    ("used", "subnet"),
    [
        ("172.17.0.0/16", "172.30.83.0/24"),
        ("172.30.0.0/16 172.17.0.0/16", "172.31.83.0/24"),
        ("172.30.83.0/24 172.31.0.0/16", "10.83.30.0/24"),
        ("fd00::/64 172.30.83.0/24", "172.31.83.0/24"),
        ("172.16.0.0/12 10.0.0.0/8 192.168.0.0/16", None),
    ],
)
def test_the_edge_network_gets_a_range_no_other_docker_network_overlaps(used, subnet, tmp_path):
    env = stub(tmp_path, "docker", f'case "$*" in "network ls -q") echo bridge ;; *) echo "{used}" ;; esac')
    result = run_part("free_subnet", tmp_path, env)
    assert (result.stdout.strip() if result.returncode == 0 else None) == subnet


def test_a_localhost_install_moves_to_free_ports_when_80_and_443_are_taken(tmp_path):
    (tmp_path / ".env").write_text("KAESO_DOMAIN=localhost\n")
    env = stub(tmp_path, "curl", 'case "$*" in *:80/*|*:443/*|*:8443/*) exit 0 ;; *) exit 7 ;; esac')
    result = run_part("domain=localhost; free_ports", tmp_path, env)
    assert result.returncode == 0, result.stderr
    assert "Kaeso answers on https://localhost:9443" in result.stderr
    assert '"9080:80"' in (tmp_path / "compose.override.yaml").read_text()
    assert '"9443:443"' in (tmp_path / "compose.override.yaml").read_text()
    assert "KAESO_PUBLIC_URL=https://localhost:9443" in (tmp_path / ".env").read_text()


def test_a_domain_install_stops_when_port_80_is_taken(tmp_path):
    (tmp_path / ".env").write_text("KAESO_DOMAIN=kaeso.example.com\n")
    env = stub(tmp_path, "curl", 'case "$*" in *:80/*) exit 0 ;; *) exit 7 ;; esac')
    result = run_part("domain=kaeso.example.com; free_ports", tmp_path, env)
    assert result.returncode == 1
    assert "Port 80 on this machine is taken by another program" in result.stderr
    assert not (tmp_path / "compose.override.yaml").exists()
