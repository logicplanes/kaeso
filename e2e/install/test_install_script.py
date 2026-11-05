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
    assert stat.S_IMODE(keys.st_mode) == 0o400
    reader = os.getuid() if sys.platform == "darwin" else 65532
    assert keys.st_uid == reader, "the gateway reads the master key as user 65532, through Docker Desktop and Colima as you"


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
