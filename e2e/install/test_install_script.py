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
                ["sh", "-s", "--", "--dir", str(work), "--domain", ":80", "--public-url", f"http://localhost:{port}"],
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
    # The gateway starts from what the installer wrote: compose.test.yaml sets no address.
    assert settings["KAESO_PUBLIC_URL"] == url
    assert settings["KAESO_SMTP_FROM"] == '"Kaeso <kaeso@localhost>"'
    assert settings["KAESO_MAIL_TRANSPORT"] == "log"
    passwords = [settings[name] for name in ("POSTGRES_PASSWORD", "POSTGRES_SUPERUSER_PASSWORD", "CLICKHOUSE_PASSWORD", "VALKEY_PASSWORD")]
    assert all(re.fullmatch(r"[0-9a-f]{48}", password) for password in passwords)
    assert len(set(passwords)) == 4

    assert settings["COMPOSE_PROFILES"] == "updater"
    assert settings["KAESO_DIR"] == str(work.resolve())
    assert re.fullmatch(r"[0-9a-f]{96}", settings["KAESO_UPDATER_TOKEN"])
    health = subprocess.run(
        ["docker", "compose", "-p", PROJECT, "ps", "--format", "{{.Service}} {{.Health}}", "updater", "docker-proxy"],
        cwd=work, capture_output=True, text=True, check=True,
    ).stdout.split("\n")
    assert sorted(line for line in health if line) == ["docker-proxy healthy", "updater healthy"], "Update in the console works at once"

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


@pytest.mark.parametrize(
    ("typed", "url"),
    [
        ("https://kaeso.example.com", "https://kaeso.example.com"),
        ("https://Kaeso.Example.com/", "https://kaeso.example.com"),
        ("http://localhost:8080", "http://localhost:8080"),
        ("kaeso.example.com", None),
        ("https://kaeso.example.com/console", None),
        ("https://kaeso.example.com:80x", None),
        ("ftp://kaeso.example.com", None),
        ("https://", None),
    ],
)
def test_a_public_url_is_a_bare_origin(typed, url, tmp_path):
    result = run_part(f"clean_public_url '{typed}'", tmp_path)
    assert (result.stdout.strip() if result.returncode == 0 else None) == url


@pytest.mark.parametrize(
    ("domain", "given", "written"),
    [
        (":80", "", ["KAESO_PUBLIC_URL=http://localhost", 'KAESO_SMTP_FROM="Kaeso <kaeso@localhost>"']),
        (":8080", "", ["KAESO_PUBLIC_URL=http://localhost:8080", 'KAESO_SMTP_FROM="Kaeso <kaeso@localhost>"']),
        (":80", "https://kaeso.example.com", ["KAESO_PUBLIC_URL=https://kaeso.example.com", 'KAESO_SMTP_FROM="Kaeso <kaeso@kaeso.example.com>"']),
        ("kaeso.example.com", "", []),
        ("kaeso.example.com", "https://ai.example.com", ["KAESO_PUBLIC_URL=https://ai.example.com"]),
    ],
)
def test_a_port_as_the_domain_gets_an_address_and_a_sender(domain, given, written, tmp_path):
    (tmp_path / ".env").write_text(f"KAESO_DOMAIN={domain}\n")
    result = run_part(f"public_url='{given}'; public_address", tmp_path)
    assert result.returncode == 0, result.stderr
    assert (tmp_path / ".env").read_text().splitlines()[1:] == written
    again = run_part("public_url=''; public_address", tmp_path)
    assert again.returncode == 0, again.stderr
    assert (tmp_path / ".env").read_text().splitlines()[1:] == written, "a second run keeps them"


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


def updater_stubs(tmp_path: Path, engine: str = "29.0.1", rootless: bool = False) -> dict[str, str]:
    options = "[name=seccomp,profile=builtin name=rootless name=cgroupns]" if rootless else "[name=seccomp,profile=builtin name=cgroupns]"
    return stub(
        tmp_path,
        "docker",
        f'''case "$*" in
  "version --format {{{{.Server.Version}}}}") echo {engine} ;;
  "info --format {{{{.SecurityOptions}}}}") echo "{options}" ;;
  "context inspect --format {{{{.Endpoints.docker.Host}}}}") echo unix:///run/user/1000/docker.sock ;;
esac''',
    )


def env_after(call: str, tmp_path: Path, env_text: str, env: dict[str, str], fresh: bool = True) -> tuple[dict[str, str], subprocess.CompletedProcess]:
    (tmp_path / ".env").write_text(env_text)
    (tmp_path / ".env").chmod(0o600)
    result = run_part(f"updater=yes; fresh={'yes' if fresh else ''}; here=/srv/kaeso; {call}", tmp_path, env)
    assert result.returncode == 0, result.stderr
    lines = (tmp_path / ".env").read_text().splitlines()
    return dict(line.split("=", 1) for line in lines), result


def test_a_new_install_gets_the_updater_so_update_in_the_console_works(tmp_path):
    settings, result = env_after("turn_on_updater", tmp_path, "KAESO_DOMAIN=localhost\n", updater_stubs(tmp_path))
    assert re.fullmatch(r"[0-9a-f]{96}", settings["KAESO_UPDATER_TOKEN"])
    assert settings["COMPOSE_PROFILES"] == "updater"
    assert settings["KAESO_DIR"] == "/srv/kaeso"
    assert "DOCKER_SOCKET" not in settings
    assert "Turning on the updater" not in result.stdout


def test_running_the_installer_again_turns_the_updater_on_and_says_so(tmp_path):
    settings, result = env_after("turn_on_updater", tmp_path, "KAESO_DOMAIN=localhost\n", updater_stubs(tmp_path), fresh=False)
    assert settings["COMPOSE_PROFILES"] == "updater"
    assert "Turning on the updater, so Update in the console installs new releases" in result.stdout


@pytest.mark.parametrize(
    ("profiles", "after"),
    [("edge", "edge,updater"), ("", "updater"), ("updater", "updater"), ("edge,updater", "edge,updater"), ("updaters", "updaters,updater")],
)
def test_the_updater_joins_the_profiles_already_on(profiles, after, tmp_path):
    settings, _ = env_after("turn_on_updater", tmp_path, f"KAESO_DOMAIN=localhost\nCOMPOSE_PROFILES={profiles}\n", updater_stubs(tmp_path))
    assert settings["COMPOSE_PROFILES"] == after
    assert stat.S_IMODE((tmp_path / ".env").stat().st_mode) == 0o600


def test_rootless_docker_gets_its_own_socket(tmp_path):
    settings, _ = env_after("turn_on_updater", tmp_path, "KAESO_DOMAIN=localhost\n", updater_stubs(tmp_path, rootless=True))
    assert settings["DOCKER_SOCKET"] == "/run/user/1000/docker.sock"


@pytest.mark.parametrize(
    ("env_text", "call", "engine"),
    [
        ("KAESO_DOMAIN=localhost\nKAESO_UPDATER_TOKEN=kept\n", "turn_on_updater", "29.0.1"),
        ("KAESO_DOMAIN=localhost\n", "updater=no; turn_on_updater", "29.0.1"),
        ("KAESO_DOMAIN=localhost\n", "turn_on_updater", "24.0.7"),
    ],
)
def test_the_updater_is_left_as_it_is(env_text, call, engine, tmp_path):
    settings, result = env_after(call, tmp_path, env_text, updater_stubs(tmp_path, engine=engine))
    assert set(settings) <= {"KAESO_DOMAIN", "KAESO_UPDATER_TOKEN"}
    assert settings.get("KAESO_UPDATER_TOKEN") in (None, "kept")
    if engine.startswith("24"):
        assert "The updater needs Docker Engine 25 or newer, and this is 24.0.7" in result.stderr


def test_the_installer_run_in_an_install_uses_that_folder(tmp_path):
    (tmp_path / "compose.yaml").write_text("services: {}\n")
    (tmp_path / ".env").write_text("KAESO_DOMAIN=localhost\n")
    assert run_part("is_install .", tmp_path).returncode == 0
    (tmp_path / ".env").write_text("OTHER=1\n")
    assert run_part("is_install .", tmp_path).returncode == 1


@pytest.mark.parametrize(
    ("a", "b", "is_newer"),
    [
        ("40.6.0", "40.6.0-rc.8", True),
        ("40.6.0-rc.8", "40.6.0", False),
        ("40.6.0-rc.10", "40.6.0-rc.9", True),
        ("40.6.1", "40.6.0", True),
        ("41.0.0", "40.99.99", True),
        ("40.6.0", "40.6.0", False),
        ("40.5.6", "40.6.0-rc.1", False),
        ("40.6.0-rc.1", "40.6.0-beta.2", True),
    ],
)
def test_versions_compare_as_semantic_versioning_says(a, b, is_newer, tmp_path):
    assert (run_part(f"newer {a} {b}", tmp_path).returncode == 0) is is_newer


def release_dir(tmp_path: Path, version: str, compose: str, digest: str | None = None, latest: bool = False) -> None:
    import hashlib
    import json

    for folder in [tmp_path / "releases" / "download" / f"v{version}"] + ([tmp_path / "releases" / "latest" / "download"] if latest else []):
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "compose.yaml").write_text(compose)
        sha = digest or hashlib.sha256(compose.encode()).hexdigest()
        (folder / "release.json").write_text(json.dumps({"schema": 1, "version": version, "compose": {"name": "compose.yaml", "sha256": sha}}, indent=2))


def compose_of(version: str) -> str:
    return f"services:\n  gateway:\n    image: ghcr.io/logicplanes/kaeso-gateway:{version}@sha256:{'a' * 64}\n"


def update_in(tmp_path: Path, version: str = "latest") -> subprocess.CompletedProcess:
    install = tmp_path / "kaeso"
    return run_part(
        f'cd "{install}"; version={version}; here="{install}"; updated=; update_compose; echo "updated=$updated"',
        tmp_path,
        {"KAESO_RELEASES_URL": (tmp_path / "releases").as_uri()},
    )


def an_install(tmp_path: Path, version: str) -> Path:
    install = tmp_path / "kaeso"
    install.mkdir()
    (install / "compose.yaml").write_text(compose_of(version))
    (install / "compose.yaml").chmod(0o660)
    return install


def test_update_installs_the_newest_stable_release_after_checking_its_hash(tmp_path):
    install = an_install(tmp_path, "40.6.0-rc.8")
    inode = (install / "compose.yaml").stat().st_ino
    release_dir(tmp_path, "40.6.0", compose_of("40.6.0"), latest=True)
    result = update_in(tmp_path)
    assert result.returncode == 0, result.stderr
    assert "Updating Kaeso in" in result.stdout and "from 40.6.0-rc.8 to 40.6.0" in result.stdout
    assert "updated=40.6.0-rc.8" in result.stdout
    assert (install / "compose.yaml").read_text() == compose_of("40.6.0")
    assert (install / "compose.yaml.previous").read_text() == compose_of("40.6.0-rc.8")
    assert (install / "compose.yaml").stat().st_ino == inode, "written in place, so the updater's mount still sees it"
    assert stat.S_IMODE((install / "compose.yaml").stat().st_mode) == 0o660
    assert sorted(p.name for p in install.iterdir()) == ["compose.yaml", "compose.yaml.previous"]


def test_update_installs_the_release_version_names(tmp_path):
    install = an_install(tmp_path, "40.6.0-rc.8")
    release_dir(tmp_path, "40.6.0-rc.9", compose_of("40.6.0-rc.9"))
    result = update_in(tmp_path, "40.6.0-rc.9")
    assert result.returncode == 0, result.stderr
    assert (install / "compose.yaml").read_text() == compose_of("40.6.0-rc.9")


def test_update_refuses_a_compose_file_that_does_not_match_the_manifest(tmp_path):
    install = an_install(tmp_path, "40.6.0-rc.8")
    release_dir(tmp_path, "40.6.0", compose_of("40.6.0"), digest="b" * 64, latest=True)
    result = update_in(tmp_path)
    assert result.returncode == 1
    assert "doesn't match its release's manifest, so the install stays as it is" in result.stderr
    assert (install / "compose.yaml").read_text() == compose_of("40.6.0-rc.8")
    assert sorted(p.name for p in install.iterdir()) == ["compose.yaml"]


def test_update_never_goes_back_to_an_older_release(tmp_path):
    install = an_install(tmp_path, "40.6.0-rc.8")
    release_dir(tmp_path, "40.5.6", compose_of("40.5.6"), latest=True)
    result = update_in(tmp_path)
    assert result.returncode == 1
    assert "Kaeso 40.6.0-rc.8 runs here, which is newer than 40.5.6" in result.stderr
    assert (install / "compose.yaml").read_text() == compose_of("40.6.0-rc.8")


def test_update_to_the_running_release_just_starts_it(tmp_path):
    install = an_install(tmp_path, "40.6.0")
    release_dir(tmp_path, "40.6.0", compose_of("40.6.0"), latest=True)
    result = update_in(tmp_path)
    assert result.returncode == 0, result.stderr
    assert "Kaeso 40.6.0 is installed in" in result.stdout
    assert "updated=\n" in result.stdout
    assert sorted(p.name for p in install.iterdir()) == ["compose.yaml"]


def test_update_that_cannot_download_changes_nothing(tmp_path):
    install = an_install(tmp_path, "40.6.0-rc.8")
    (tmp_path / "releases").mkdir()
    result = update_in(tmp_path, "9.9.9")
    assert result.returncode == 1
    assert "Couldn't download" in result.stderr
    assert sorted(p.name for p in install.iterdir()) == ["compose.yaml"]


def check_settings_in(tmp_path: Path, answer: str, code: int) -> subprocess.CompletedProcess:
    (tmp_path / "answer").write_text(answer + "\n")
    fake = f'docker() {{ echo "docker $*" >>"{tmp_path}/calls"; cat "{tmp_path}/answer"; return {code}; }}'
    return run_part(f'{fake}; cd "{tmp_path / "kaeso"}"; fresh=; target=40.7.0; updated=40.6.0; check_settings; echo checked', tmp_path)


def test_an_update_whose_gateway_refuses_the_settings_keeps_the_running_release(tmp_path):
    install = an_install(tmp_path, "40.7.0")
    (install / "compose.yaml.previous").write_text(compose_of("40.6.0"))
    refusal = "Error: configuration: KAESO_KPMR_INTERVAL (kpmr.interval) is 1m, and it must be `auto` or at least 5m"
    result = check_settings_in(tmp_path, refusal, 1)
    assert result.returncode == 1
    assert refusal in result.stderr
    assert "Kaeso 40.7.0 refused this install's settings, so this install keeps running 40.6.0" in result.stderr
    assert (install / "compose.yaml").read_text() == compose_of("40.6.0")
    assert (tmp_path / "calls").read_text() == "docker compose run --rm --no-deps -T gateway config check\n"


def refused_update_in(tmp_path: Path) -> subprocess.CompletedProcess:
    (tmp_path / "answer").write_text("Error: configuration: KAESO_KPMR_INTERVAL (kpmr.interval) is 1m\n")
    fake = f'docker() {{ cat "{tmp_path}/answer"; return 1; }}'
    install = tmp_path / "kaeso"
    return run_part(
        f'{fake}; cd "{install}"; version=40.7.0; here="{install}"; fresh=; updated=; update_compose; check_settings',
        tmp_path,
        {"KAESO_RELEASES_URL": (tmp_path / "releases").as_uri()},
    )


def test_a_refused_update_leaves_no_compose_backup_behind(tmp_path):
    install = an_install(tmp_path, "40.6.0")
    release_dir(tmp_path, "40.7.0", compose_of("40.7.0"))
    result = refused_update_in(tmp_path)
    assert result.returncode == 1
    assert "keeps running 40.6.0" in result.stderr
    assert (install / "compose.yaml").read_text() == compose_of("40.6.0")
    assert sorted(p.name for p in install.iterdir()) == ["compose.yaml"]


def test_a_refused_update_keeps_the_compose_file_an_earlier_update_left(tmp_path):
    install = an_install(tmp_path, "40.6.0")
    earlier = (compose_of("40.5.0") + "\n\n").encode()
    (install / "compose.yaml.previous").write_bytes(earlier)
    (install / "compose.yaml.previous").chmod(0o640)
    inode = (install / "compose.yaml.previous").stat().st_ino
    release_dir(tmp_path, "40.7.0", compose_of("40.7.0"))
    result = refused_update_in(tmp_path)
    assert result.returncode == 1
    assert (install / "compose.yaml").read_text() == compose_of("40.6.0")
    assert (install / "compose.yaml.previous").read_bytes() == earlier
    assert (install / "compose.yaml.previous").stat().st_ino == inode
    assert stat.S_IMODE((install / "compose.yaml.previous").stat().st_mode) == 0o640
    assert sorted(p.name for p in install.iterdir()) == ["compose.yaml", "compose.yaml.previous"]


@pytest.mark.parametrize(
    ("answer", "code"),
    [("The configuration is valid.", 0), ("error: unrecognized subcommand 'config'", 2)],
)
def test_an_update_whose_gateway_takes_the_settings_or_cannot_check_them_goes_on(answer, code, tmp_path):
    install = an_install(tmp_path, "40.7.0")
    result = check_settings_in(tmp_path, answer, code)
    assert result.returncode == 0, result.stderr
    assert "checked" in result.stdout
    assert (install / "compose.yaml").read_text() == compose_of("40.7.0")


def test_a_fresh_install_checks_no_settings(tmp_path):
    (tmp_path / "kaeso").mkdir()
    result = run_part(f'docker() {{ echo called >"{tmp_path}/calls"; }}; fresh=yes; updated=; check_settings; echo checked', tmp_path)
    assert result.returncode == 0, result.stderr
    assert not (tmp_path / "calls").exists()


def test_starting_an_install_again_checks_its_settings_and_refuses_without_touching_its_files(tmp_path):
    install = an_install(tmp_path, "40.7.0")
    (install / "compose.yaml.previous").write_text(compose_of("40.6.0"))
    refusal = "Error: configuration: KAESO_SMTP_USER (smtp.user) is set without KAESO_SMTP_PASSWORD (smtp.password)"
    (tmp_path / "answer").write_text(refusal + "\n")
    fake = f'docker() {{ echo "docker $*" >>"{tmp_path}/calls"; cat "{tmp_path}/answer"; return 1; }}'
    result = run_part(f'{fake}; cd "{install}"; fresh=; updated=; check_settings; echo checked', tmp_path)
    assert result.returncode == 1
    assert refusal in result.stderr
    assert "Kaeso refused this install's settings, so the installer doesn't start it." in result.stderr
    assert "checked" not in result.stdout
    assert (install / "compose.yaml").read_text() == compose_of("40.7.0")
    assert (install / "compose.yaml.previous").read_text() == compose_of("40.6.0")


def gateway_running(tmp_path: Path, image: str | None) -> str:
    """A docker function whose install has a gateway container from that image, or none."""
    container = "echo c0ffee" if image else ":"
    return f'''docker() {{
  case "$*" in
    "compose ps -a -q gateway") {container} ;;
    "inspect --format {{{{.Config.Image}}}} c0ffee") echo "{image}" ;;
    *) return 1 ;;
  esac
}}'''


def gateway_image(version: str) -> str:
    return f"ghcr.io/logicplanes/kaeso-gateway:{version}@sha256:{'a' * 64}"


@pytest.mark.parametrize(
    ("previous", "image", "unfinished"),
    [
        ("40.6.0", gateway_image("40.6.0"), True),
        ("40.6.0", gateway_image("40.7.0"), False),
        (None, gateway_image("40.6.0"), False),
        ("40.5.0", gateway_image("40.6.0"), False),
        ("40.6.0", None, False),
        ("40.6.0", "kaeso-gateway:local", False),
    ],
)
def test_an_update_that_stopped_before_its_release_started_counts_as_unfinished(previous, image, unfinished, tmp_path):
    install = an_install(tmp_path, "40.7.0")
    if previous:
        (install / "compose.yaml.previous").write_text(compose_of(previous))
    result = run_part(
        f'{gateway_running(tmp_path, image)}; cd "{install}"; here="{install}"; updated=; '
        'if unfinished_update; then echo "updated=$updated target=$target"; else echo finished; fi',
        tmp_path,
    )
    assert result.returncode == 0, result.stderr
    if unfinished:
        assert "Updating Kaeso in" in result.stdout and "from 40.6.0 to 40.7.0" in result.stdout
        assert "updated=40.6.0 target=40.7.0" in result.stdout
    else:
        assert result.stdout.strip() == "finished"


def compose_with_console(version: str) -> str:
    return compose_of(version) + f"  console:\n    image: ghcr.io/logicplanes/kaeso-console:{version}@sha256:{'b' * 64}\n"


def docker_for_updates(tmp_path: Path, state: Path) -> dict[str, str]:
    """A Docker engine with one Kaeso install: its gateway container, the images it has, a registry that
    can be offline, and a 40.7.0 gateway that refuses KAESO_KPMR_INTERVAL=1m."""
    return stub(
        tmp_path,
        "docker",
        f'''S="{state}"
echo "$*" >>"$S/calls"
case "$*" in
  info | "info --format {{{{.SecurityOptions}}}}") ;;
  "info --format {{{{.Architecture}}}}") echo x86_64 ;;
  "info --format {{{{.MemTotal}}}}") echo 8000000000 ;;
  "compose version --short") echo 2.40.0 ;;
  "ps -a --filter"*) ;;
  "compose ps -a -q gateway") [ ! -s "$S/started" ] || echo c0ffee ;;
  "inspect --format {{{{.Config.Image}}}} c0ffee") cat "$S/started" ;;
  "compose ps -q --status running caddy") echo caddy ;;
  "compose config --images") sed -n 's/^ *image: //p' compose.yaml ;;
  "image inspect "*) grep -qxF "$3" "$S/pulled" ;;
  "compose pull"*)
    [ ! -f "$S/offline" ] || {{ echo "Error response from daemon: Get https://ghcr.io/v2/: net/http: TLS handshake timeout" >&2; exit 1; }}
    sed -n 's/^ *image: //p' compose.yaml >>"$S/pulled" ;;
  "run --rm --user 0:0 -v "*) ;;
  "compose run --rm --no-deps -T gateway config check")
    if grep -q 'kaeso-gateway:40.7.0' compose.yaml && grep -q '^KAESO_KPMR_INTERVAL=1m$' .env; then
      echo 'Error: configuration: KAESO_KPMR_INTERVAL (kpmr.interval) is 1m, and it must be auto or at least 5m' >&2
      exit 1
    fi
    echo "The configuration is valid." ;;
  "compose up "*) sed -n 's/^ *image: \\(.*kaeso-gateway.*\\)/\\1/p' compose.yaml >"$S/started" ;;
  "compose exec -T gateway kaeso setup-link") exit 1 ;;
  *) echo "unexpected: docker $*" >&2; exit 99 ;;
esac''',
    )


@pytest.mark.parametrize("again", [["--update"], []], ids=["update", "plain"])
def test_an_update_whose_pull_failed_checks_the_settings_when_run_again(again, tmp_path):
    install = an_install(tmp_path, "40.6.0")
    (install / "compose.yaml").write_text(compose_with_console("40.6.0"))
    (install / ".env").write_text(
        "KAESO_DOMAIN=:80\nKAESO_PUBLIC_URL=http://localhost\nEDGE_SUBNET=172.31.83.0/24\n"
        "KAESO_UPDATER_TOKEN=kept\nCOMPOSE_PROFILES=updater\nKAESO_KPMR_INTERVAL=1m\n"
    )
    (install / "master-keys").write_text("kept\n")
    release_dir(tmp_path, "40.7.0", compose_with_console("40.7.0"), latest=True)
    state = tmp_path / "state"
    state.mkdir()
    (state / "pulled").write_text("".join(f"{line.split('image: ')[1]}\n" for line in compose_with_console("40.6.0").splitlines() if "image: " in line))
    (state / "started").write_text(gateway_image("40.6.0") + "\n")
    env = {**docker_for_updates(tmp_path, state), "KAESO_RELEASES_URL": (tmp_path / "releases").as_uri(), "COMPOSE_PROJECT_NAME": "kaeso"}

    def install_with(*args: str) -> subprocess.CompletedProcess:
        with SCRIPT.open() as script:
            return subprocess.run(
                ["sh", "-s", "--", "--dir", str(install), "--no-open", *args],
                stdin=script, env={**os.environ, **env}, capture_output=True, text=True, timeout=60, start_new_session=True,
            )

    (state / "offline").touch()
    first = install_with("--update")
    assert first.returncode == 1, first.stdout + first.stderr
    assert "Couldn't download Kaeso's images" in first.stderr
    assert (install / "compose.yaml").read_text() == compose_with_console("40.7.0")

    (state / "offline").unlink()
    refused = install_with(*again)
    assert refused.returncode == 1, refused.stdout + refused.stderr
    assert "from 40.6.0 to 40.7.0" in refused.stdout
    assert "KAESO_KPMR_INTERVAL (kpmr.interval) is 1m" in refused.stderr
    assert "Kaeso 40.7.0 refused this install's settings, so this install keeps running 40.6.0" in refused.stderr
    assert (install / "compose.yaml").read_text() == compose_with_console("40.6.0")
    assert not (install / "compose.yaml.previous").exists(), "it was a copy of the compose.yaml that runs"
    assert (state / "started").read_text().strip() == gateway_image("40.6.0")
    assert not any(line.startswith("compose up") for line in (state / "calls").read_text().splitlines())

    (install / ".env").write_text((install / ".env").read_text().replace("KAESO_KPMR_INTERVAL=1m\n", "KAESO_KPMR_INTERVAL=5m\n"))
    fixed = install_with("--update")
    assert fixed.returncode == 0, fixed.stdout + fixed.stderr
    assert "from 40.6.0 to 40.7.0" in fixed.stdout
    assert "Kaeso is running at http://localhost" in fixed.stdout
    assert (install / "compose.yaml").read_text() == compose_with_console("40.7.0")
    assert (install / "compose.yaml.previous").read_text() == compose_with_console("40.6.0")
    assert (state / "started").read_text().strip() == gateway_image("40.7.0")
