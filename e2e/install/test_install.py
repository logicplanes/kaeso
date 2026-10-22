import json
import os
import re
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path
from typing import Callable, NamedTuple

import anthropic
import httpx
import openai
import pytest

ROOT = Path(__file__).resolve().parents[2]
PROJECT = "kaeso-install-test"
SETUP_LINK = re.compile(r"#token=(\S+)")
UNREACHABLE = "Kaeso can't be reached right now; try again in a minute."
UPDATING = "Kaeso is being updated; try again in a minute."


class Install(NamedTuple):
    url: str
    token: str
    env: dict[str, str]
    run: Callable[..., subprocess.CompletedProcess]
    health: dict[str, str]
    work: Path
    console: str


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def image(variable: str, target: str) -> str:
    if os.environ.get(variable):
        return os.environ[variable]
    tag = f"kaeso-{target}:install-test"
    dockerfile = ROOT / "deploy" / "gateway.Dockerfile"
    subprocess.run(["docker", "build", "-f", str(dockerfile), "--target", target, "-t", tag, str(ROOT)], check=True)
    return tag


def built(variable: str, name: str) -> str:
    if os.environ.get(variable):
        return os.environ[variable]
    tag = f"kaeso-{name}:install-test"
    dockerfile = ROOT / "deploy" / f"{name}.Dockerfile"
    subprocess.run(["docker", "build", "-f", str(dockerfile), "-t", tag, str(ROOT)], check=True)
    return tag


def poll(fetch, what: str, seconds: float = 120):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        found = fetch()
        if found:
            return found
        time.sleep(1)
    raise TimeoutError(f"{what} did not happen within {seconds} seconds")


@pytest.fixture(scope="module")
def install(tmp_path_factory):
    work = tmp_path_factory.mktemp("install")
    shutil.copy(ROOT / "deploy" / "compose.yaml", work / "compose.yaml")
    shutil.copy(Path(__file__).with_name("compose.test.yaml"), work / "compose.test.yaml")
    port = free_port()
    gateway = image("KAESO_TEST_GATEWAY_IMAGE", "gateway")
    mock = image("KAESO_TEST_MOCK_IMAGE", "mock")
    console = built("KAESO_TEST_CONSOLE_IMAGE", "console")
    updater = built("KAESO_TEST_UPDATER_IMAGE", "updater")
    (work / "master-keys").write_text(
        subprocess.check_output(["docker", "run", "--rm", gateway, "master-key", "generate"], text=True)
    )
    env = {
        "KAESO_DOMAIN": ":80",
        "POSTGRES_PASSWORD": os.urandom(16).hex(),
        "POSTGRES_SUPERUSER_PASSWORD": os.urandom(16).hex(),
        "CLICKHOUSE_PASSWORD": os.urandom(16).hex(),
        "VALKEY_PASSWORD": os.urandom(16).hex(),
        "KAESO_TEST_PORT": str(port),
        "KAESO_TEST_GATEWAY_IMAGE": gateway,
        "KAESO_TEST_MOCK_IMAGE": mock,
        "KAESO_TEST_CONSOLE_IMAGE": console,
        "KAESO_TEST_UPDATER_IMAGE": updater,
        "KAESO_UPDATER_TOKEN": os.urandom(32).hex(),
        "COMPOSE_PROFILES": "updater",
        "KAESO_DIR": str(work),
    }
    (work / ".env").write_text("".join(f"{name}={value}\n" for name, value in env.items()))
    compose = ["docker", "compose", "-p", PROJECT, "-f", "compose.yaml", "-f", "compose.test.yaml"]

    def run(*args: str, check: bool = True, **kwargs) -> subprocess.CompletedProcess:
        return subprocess.run([*compose, *args], cwd=work, check=check, text=True, **kwargs)

    url = f"http://localhost:{port}"
    try:
        run("up", "-d", "--wait", "--wait-timeout", "300")
        listed = run("ps", "--all", "--format", "json", capture_output=True).stdout
        health = {s["Service"]: s["Health"] for s in map(json.loads, listed.splitlines())}

        printed = run("exec", "-T", "gateway", "kaeso", "setup-link", capture_output=True).stdout
        token = SETUP_LINK.search(printed).group(1)
        logs = run("logs", "gateway", capture_output=True).stdout
        assert "kaeso setup-link" in logs, "the gateway's log points to the setup link command"
        assert token not in logs, "the setup token reached the gateway's log"
        yield Install(url, token, env, run, health, work, console)
    finally:
        subprocess.run([*compose, "logs", "--no-color"], cwd=work, stdout=open(work / "compose.log", "w"))
        subprocess.run([*compose, "down", "-v", "--remove-orphans"], cwd=work, check=False)


@pytest.fixture(scope="module")
def owner(install) -> httpx.Client:
    http = httpx.Client(base_url=install.url, headers={"origin": install.url}, timeout=30)
    r = http.post(
        "/platform/v1/auth/setup",
        json={
            "token": install.token,
            "email": "owner@example.com",
            "name": "Owner",
            "password": "correct horse battery",
            "org_name": "First org",
        },
    )
    assert r.status_code == 200, r.text
    return http


def test_a_fresh_install_serves_both_sdks_and_records_usage(install, owner):
    url = install.url
    http = owner

    def post(path: str, body: dict) -> dict:
        r = http.post(f"/platform/v1{path}", json=body)
        assert r.status_code in (200, 201), f"{path}: {r.status_code} {r.text}"
        return r.json()

    org = http.get("/platform/v1/auth/session").json()["memberships"][0]["org_id"]
    project = post(f"/orgs/{org}/projects", {"name": "First project"})["id"]
    base = f"/orgs/{org}/projects/{project}"
    post(f"/orgs/{org}/credentials", {"provider": "openai", "name": "OpenAI", "secret": "sk-test-install-openai"})
    post(f"/orgs/{org}/credentials", {"provider": "anthropic", "name": "Anthropic", "secret": "sk-ant-test-install"})
    post(f"{base}/models", {"provider": "openai", "model_id": "gpt-4o"})
    post(f"{base}/models", {"provider": "anthropic", "model_id": "claude-haiku-4-5"})
    key = post(f"{base}/keys", {"name": "first"})["secret"]

    oa = openai.OpenAI(base_url=f"{url}/v1", api_key=key, max_retries=0)
    reply = oa.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}])
    assert reply.choices[0].message.content
    an = anthropic.Anthropic(base_url=url, api_key=key, max_retries=0)
    message = an.messages.create(
        model="claude-haiku-4-5", max_tokens=50, messages=[{"role": "user", "content": "Hello"}]
    )
    assert message.content

    def recorded() -> bool:
        logs = http.get(f"/platform/v1{base}/logs").json()
        usage = http.get(f"/platform/v1{base}/usage?group_by=model").json()
        return len(logs.get("data", [])) == 2 and sum(r["requests"] for r in usage.get("data", [])) == 2

    poll(recorded, "two log entries and their usage", seconds=30)


def test_wait_returns_once_every_service_is_healthy_caddy_included(install):
    shipped = ("caddy", "gateway", "console", "postgres", "clickhouse", "valkey")
    assert {name: install.health.get(name) for name in shipped} == dict.fromkeys(shipped, "healthy")


def test_everything_outside_the_apis_is_the_console(install):
    page = httpx.get(f"{install.url}/", timeout=10)
    assert page.status_code == 200
    assert "text/html" in page.headers["content-type"]


def test_the_gateway_gets_no_more_database_rights_than_it_needs(install):
    def query(*args: str, check: bool = True) -> subprocess.CompletedProcess:
        return install.run("exec", "-T", *args, check=check, capture_output=True)

    role = "SELECT rolsuper FROM pg_roles WHERE rolname = current_user"
    superuser = query("postgres", "psql", "-U", "kaeso", "-d", "kaeso", "-tAc", role)
    assert superuser.stdout.strip() == "f"
    client = ["clickhouse-client", "--user", "kaeso", "--password", install.env["CLICKHOUSE_PASSWORD"]]
    create_user = query("clickhouse", *client, "--query", "CREATE USER intruder", check=False)
    assert create_user.returncode != 0
    assert "ACCESS_DENIED" in create_user.stderr
    anonymous = query("-e", "VALKEYCLI_AUTH=", "valkey", "valkey-cli", "ping")
    assert "NOAUTH" in anonymous.stdout


def test_the_updater_reaches_docker_only_through_its_proxy(install, owner):
    assert {name: install.health.get(name) for name in ("updater", "docker-proxy")} == {
        "updater": "healthy",
        "docker-proxy": "healthy",
    }
    status = owner.get("/platform/v1/instance/updates")
    assert status.status_code == 200, status.text
    assert status.json()["updater"]["state"] == "ready", status.json()
    assert status.json()["settings"]["mode"] == "manual"
    stranger = subprocess.run(
        ["docker", "run", "--rm", "--network", f"{PROJECT}_docker", "--entrypoint", "wget", install.console,
         "-qO-", "-T", "5", "http://docker-proxy:2375/v1.44/version"],
        capture_output=True,
        text=True,
    )
    assert stranger.returncode != 0, stranger.stdout


def test_the_updater_writes_its_files_without_root(install):
    inspected = subprocess.run(
        ["docker", "inspect", "--format", "{{.Config.User}}", f"{PROJECT}-updater-1"], capture_output=True, text=True, check=True
    )
    assert inspected.stdout.strip() == "65532:65532"

    def as_updater(script: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            ["docker", "run", "--rm", "--user", "65532:65532", "--network", "none",
             "-v", f"{install.work / 'compose.yaml'}:/kaeso/compose.yaml",
             "-v", f"{PROJECT}_maintenance:/maintenance", "-v", f"{PROJECT}_updater:/state",
             "--entrypoint", "sh", install.console, "-c", script],
            capture_output=True,
            text=True,
        )

    volumes = as_updater("touch /maintenance/probe /state/probe && rm /maintenance/probe /state/probe")
    assert volumes.returncode == 0, volumes.stderr
    group, mode = as_updater("stat -c '%g %A' /kaeso/compose.yaml").stdout.split()
    assert mode[4:6] == "rw", mode
    docker_runs_in_a_vm_that_shares_this_folder = sys.platform == "darwin"
    if docker_runs_in_a_vm_that_shares_this_folder:
        return
    assert group == "65532"
    written = as_updater("test -w /kaeso/compose.yaml")
    assert written.returncode == 0, written.stderr


def test_the_init_services_run_again_over_directories_only_their_services_can_read(install):
    for volume in ("caddy_data", "maintenance", "updater"):
        subprocess.run(
            ["docker", "run", "--rm", "--user", "65532:65532", "--network", "none", "-v", f"{PROJECT}_{volume}:/v",
             "--entrypoint", "sh", install.console, "-c", "mkdir -p -m 700 /v/private/inner && touch /v/private/inner/file"],
            check=True,
        )
    for service in ("caddy-init", "updater-init"):
        again = install.run("run", "--rm", "--no-deps", service, check=False, capture_output=True)
        assert again.returncode == 0, again.stderr


def test_the_data_services_start_again_over_the_data_they_wrote(install, owner):
    for service in ("valkey", "postgres", "clickhouse"):
        again = install.run("up", "-d", "--wait", "--no-deps", "--force-recreate", service, check=False, capture_output=True)
        assert again.returncode == 0, f"{service}: {again.stderr}"


def flag(install: Install, state: str | None) -> None:
    command = f"printf '%s' '{state}' > /m/active" if state else "rm -f /m/active"
    subprocess.run(
        ["docker", "run", "--rm", "--user", "65532:65532", "-v", f"{PROJECT}_maintenance:/m", "--entrypoint", "sh", install.console, "-c", command],
        check=True,
    )


def test_the_edge_answers_every_api_in_its_format_while_kaeso_is_away(install, owner):
    url = install.url
    install.run("stop", "gateway", "console")
    try:
        for maintenance, message in [(None, UNREACHABLE), ('{"state":"updating","step":"migrating"}', UPDATING)]:
            flag(install, maintenance)
            platform = httpx.get(f"{url}/platform/v1/auth/session", timeout=10)
            assert platform.status_code == 503
            assert platform.headers["retry-after"] == "30"
            assert platform.json() == {"error": {"type": "unavailable", "message": message}}

            oa = openai.OpenAI(base_url=f"{url}/v1", api_key="sk-kso-test", max_retries=0)
            with pytest.raises(openai.APIStatusError) as raised:
                oa.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}])
            assert raised.value.status_code == 503
            assert raised.value.body["message"] == message
            assert raised.value.body["type"] == "service_unavailable_error"

            an = anthropic.Anthropic(base_url=url, api_key="sk-kso-test", max_retries=0)
            with pytest.raises(anthropic.APIStatusError) as raised:
                an.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=[{"role": "user", "content": "Hello"}])
            assert raised.value.status_code == 529
            assert raised.value.body["error"] == {"type": "overloaded_error", "message": message}

            page = httpx.get(f"{url}/settings/instance/updates", headers={"accept": "text/html"}, timeout=10)
            assert page.status_code == 503
            assert page.headers["content-type"].startswith("text/html")
            assert "default-src 'none'" in page.headers["content-security-policy"]
            assert "Kaeso" in page.text
            status = httpx.get(f"{url}/kaeso-maintenance.json", timeout=10)
            if maintenance:
                assert status.json() == {"state": "updating", "step": "migrating"}
            else:
                assert status.status_code == 404
    finally:
        flag(install, None)
        install.run("start", "gateway", "console")

    def back() -> bool:
        try:
            return httpx.get(f"{url}/platform/v1/instance/updates", cookies=owner.cookies, timeout=5).status_code == 200
        except httpx.HTTPError:
            return False

    poll(back, "the gateway answering again", seconds=120)
