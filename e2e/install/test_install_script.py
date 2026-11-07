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


@pytest.mark.parametrize(
    ("network_exists", "written"),
    [(False, "EDGE_SUBNET=172.31.83.0/24"), (True, None)],
)
def test_an_install_that_never_started_moves_its_edge_network_off_a_taken_range(network_exists, written, tmp_path):
    (tmp_path / ".env").write_text("KAESO_DOMAIN=localhost\n")
    inspect = "exit 0" if network_exists else "exit 1"
    env = stub(
        tmp_path,
        "docker",
        f'case "$*" in "network inspect kaeso_edge") {inspect} ;; "network ls -q") echo bridge ;; *) echo "172.30.0.0/16" ;; esac',
    )
    result = run_part("pin_subnet kaeso", tmp_path, env)
    assert result.returncode == 0, result.stderr
    lines = (tmp_path / ".env").read_text().splitlines()
    assert [line for line in lines if line.startswith("EDGE_SUBNET=")] == ([written] if written else [])


def run_detached(call: str, tmp_path: Path, env: dict[str, str], stdout_tty: bool = False) -> tuple[int, str, str]:
    """Runs one of the installer's functions without a controlling terminal, so it can't ask, optionally writing to a terminal."""
    functions = SCRIPT.read_text().rsplit("main \"$@\"", 1)[0]
    full_env = {key: value for key, value in {**os.environ, **env}.items() if key != "SSH_CONNECTION" or "SSH_CONNECTION" in env}
    if not stdout_tty:
        result = subprocess.run(
            ["sh", "-c", f"{functions}\n{call}"], cwd=tmp_path, env=full_env, capture_output=True, text=True, start_new_session=True,
        )
        return result.returncode, result.stdout, result.stderr
    import pty

    leader, follower = pty.openpty()
    proc = subprocess.Popen(
        ["sh", "-c", f"{functions}\n{call}"], cwd=tmp_path, env=full_env, stdout=follower, stderr=subprocess.PIPE, text=True, start_new_session=True,
    )
    os.close(follower)
    output = b""
    while True:
        try:
            chunk = os.read(leader, 4096)
        except OSError:
            break
        if not chunk:
            break
        output += chunk
    os.close(leader)
    stderr = proc.stderr.read() if proc.stderr else ""
    return proc.wait(), output.decode(), stderr


def engine_stubs(tmp_path: Path, context: str, starter: str) -> dict[str, str]:
    """A stopped Docker engine on a Mac: docker info fails until the starter has run."""
    started = tmp_path / "started"
    env = stub(tmp_path, "uname", "echo Darwin")
    stub(tmp_path, "sleep", ":")
    stub(
        tmp_path,
        "docker",
        f'case "$*" in "context show") echo {context} ;; info*) [ -f "{started}" ] || {{ echo "Cannot connect to the Docker daemon" >&2; exit 1; }} ;; esac',
    )
    stub(tmp_path, starter, f'echo "$*" >> "{tmp_path}/calls"; touch "{started}"')
    return env


@pytest.mark.parametrize(
    ("context", "starter", "said", "called"),
    [
        ("orbstack", "orb", "Starting OrbStack", "start"),
        ("colima", "colima", "Starting Colima", "start"),
        ("colima-work", "colima", "Starting Colima", "start --profile work"),
        ("desktop-linux", "open", "Starting Docker Desktop", "-a Docker"),
    ],
)
def test_a_stopped_docker_engine_on_a_mac_is_started(context, starter, said, called, tmp_path):
    env = engine_stubs(tmp_path, context, starter)
    code, out, err = run_detached("start_docker", tmp_path, env)
    assert code == 0, err
    assert said in out
    assert (tmp_path / "calls").read_text().strip() == called


def test_an_engine_that_never_comes_up_lets_the_installer_explain(tmp_path):
    env = engine_stubs(tmp_path, "orbstack", "orb")
    stub(tmp_path, "orb", ":")
    code, out, _ = run_detached("start_docker", tmp_path, env)
    assert code == 1
    assert "Starting OrbStack" in out


def test_a_mac_without_a_known_engine_is_not_touched(tmp_path):
    env = stub(tmp_path, "uname", "echo Darwin")
    stub(tmp_path, "docker", 'case "$*" in "context show") echo default ;; *) exit 1 ;; esac')
    code, out, _ = run_detached(f'HOME="{tmp_path}"; PATH="{tmp_path}/bin:/usr/bin:/bin"; [ -d /Applications/Docker.app ] && exit 3; start_docker', tmp_path, env)
    if code == 3:
        pytest.skip("this Mac has Docker Desktop in /Applications")
    assert code == 1
    assert out == ""


@pytest.mark.parametrize(
    ("domain", "own"),
    [
        ("localhost", True),
        ("kaeso.localhost", True),
        ("kaeso.internal", True),
        ("printer.local", True),
        ("nas.home.arpa", True),
        (":80", True),
        ("203.0.113.7", True),
        ("kaeso.example.com", False),
        ("203-0-113-7.sslip.io", False),
    ],
)
def test_caddy_signs_its_own_certificate_only_for_local_names_and_addresses(domain, own, tmp_path):
    assert (run_part(f"own_certificate '{domain}'", tmp_path).returncode == 0) is own


def test_a_domain_without_a_dns_record_is_reported_before_installing(tmp_path):
    env = stub(tmp_path, "getent", "exit 2")
    code, _, err = run_detached("check_dns kaeso.example.com", tmp_path, env)
    assert code == 0
    assert "kaeso.example.com has no DNS record yet" in err


def test_a_domain_with_a_dns_record_and_localhost_pass_quietly(tmp_path):
    env = stub(tmp_path, "getent", '[ "$2" = kaeso.example.com ]')
    for domain in ("kaeso.example.com", "localhost"):
        code, _, err = run_detached(f"check_dns {domain}", tmp_path, env)
        assert (code, err) == (0, "")


def test_the_certificate_check_asks_caddy_on_this_machine(tmp_path):
    (tmp_path / ".env").write_text("KAESO_DOMAIN=kaeso.example.com\n")
    env = stub(tmp_path, "curl", f'echo "$*" >> "{tmp_path}/calls"')
    code, _, err = run_detached("domain=kaeso.example.com; certificate_ready", tmp_path, env)
    assert code == 0, err
    assert "--resolve kaeso.example.com:443:127.0.0.1 https://kaeso.example.com/healthz" in (tmp_path / "calls").read_text()


def test_a_missing_certificate_is_explained_with_caddys_reason(tmp_path):
    (tmp_path / ".env").write_text("KAESO_DOMAIN=kaeso.example.com\n")
    env = stub(tmp_path, "curl", "exit 60")
    stub(tmp_path, "sleep", ":")
    stub(
        tmp_path,
        "docker",
        "echo '{\"level\":\"error\",\"msg\":\"could not get certificate from issuer\",\"error\":\"acme: challenge failed: connection refused\"}'",
    )
    code, _, err = run_detached("domain=kaeso.example.com; here=/srv/kaeso; certificate_ready || explain_certificate", tmp_path, env)
    assert code == 0
    assert "Caddy hasn't got a certificate for kaeso.example.com yet" in err
    assert "The certificate authority answered: acme: challenge failed: connection refused" in err
    assert "ports 80 and 443 are open to the internet" in err


def test_a_localhost_install_skips_the_certificate_check(tmp_path):
    (tmp_path / ".env").write_text("KAESO_DOMAIN=localhost\n")
    env = stub(tmp_path, "curl", "exit 60")
    code, _, _ = run_detached("domain=localhost; certificate_ready", tmp_path, env)
    assert code == 0


@pytest.mark.parametrize(
    ("setting", "opened"),
    [("open_browser=yes", True), ("open_browser=no", False), ("open_browser=yes; SSH_CONNECTION='203.0.113.9 5000 203.0.113.7 22'", False)],
)
def test_the_setup_link_opens_in_the_browser_on_this_computer(setting, opened, tmp_path):
    env = stub(tmp_path, "uname", "echo Darwin")
    stub(tmp_path, "open", f'echo "$*" > "{tmp_path}/opened"')
    code, out, err = run_detached(f"{setting}; open_link 'https://localhost/setup#token=stp_1'", tmp_path, env, stdout_tty=True)
    assert code == 0, err
    assert (tmp_path / "opened").exists() is opened
    if opened:
        assert (tmp_path / "opened").read_text().strip() == "https://localhost/setup#token=stp_1"
        assert "It's open in your browser." in out


def test_the_setup_link_stays_closed_when_the_output_is_not_a_terminal(tmp_path):
    env = stub(tmp_path, "uname", "echo Darwin")
    stub(tmp_path, "open", f'touch "{tmp_path}/opened"')
    code, _, _ = run_detached("open_browser=yes; open_link 'https://localhost/setup#token=stp_1'", tmp_path, env)
    assert code == 0
    assert not (tmp_path / "opened").exists()


def test_no_open_is_an_option(tmp_path):
    with SCRIPT.open() as script:
        result = subprocess.run(["sh", "-s", "--", "--help"], stdin=script, capture_output=True, text=True)
    assert "--no-open" in result.stdout
