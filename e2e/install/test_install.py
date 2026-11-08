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


def image_id(reference: str) -> str:
    """docker-proxy lets Compose inspect only images named by digest or id, and local builds have no digest."""
    return subprocess.check_output(["docker", "image", "inspect", "--format", "{{.Id}}", reference], text=True).strip()


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
    gateway = image_id(image("KAESO_TEST_GATEWAY_IMAGE", "gateway"))
    mock = image_id(image("KAESO_TEST_MOCK_IMAGE", "mock"))
    console = built("KAESO_TEST_CONSOLE_IMAGE", "console")
    updater = image_id(built("KAESO_TEST_UPDATER_IMAGE", "updater"))
    (work / "master-keys").write_text(
        subprocess.check_output(["docker", "run", "--rm", gateway, "master-key", "generate"], text=True)
    )
    subprocess.run(
        ["docker", "run", "--rm", "--user", "0:0", "-v", f"{work}:/kaeso", "--entrypoint", "sh", console,
         "-c", "chown 65532:65532 /kaeso/master-keys && chmod 0400 /kaeso/master-keys"],
        check=True,
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
        "KAESO_TEST_CONSOLE_IMAGE": image_id(console),
        "KAESO_TEST_UPDATER_IMAGE": updater,
        "KAESO_UPDATER_TOKEN": os.urandom(32).hex(),
        "COMPOSE_PROFILES": "updater",
        "KAESO_DIR": str(work),
        "GATEWAY_REPLICAS": "2",
        "COMPOSE_FILE": "compose.yaml:compose.test.yaml",
    }
    (work / ".env").touch(mode=0o600)
    (work / ".env").write_text("".join(f"{name}={value}\n" for name, value in env.items()))
    compose = ["docker", "compose", "-p", PROJECT, "-f", "compose.yaml", "-f", "compose.test.yaml"]
    # .env alone configures the install, as for the updater's compose runner: a variable left in this
    # process's environment, such as CI's image tags, would win over it and change the services' hashes.
    without_overrides = {name: value for name, value in os.environ.items() if name not in env}

    def run(*args: str, check: bool = True, **kwargs) -> subprocess.CompletedProcess:
        return subprocess.run([*compose, *args], cwd=work, check=check, text=True, env=without_overrides, **kwargs)

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
        subprocess.run([*compose, "logs", "--no-color"], cwd=work, env=without_overrides, stdout=open(work / "compose.log", "w"))
        subprocess.run([*compose, "down", "-v", "--remove-orphans"], cwd=work, env=without_overrides, check=False)


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


def test_agents_reach_the_mcp_server_and_its_sign_in_through_the_proxy(install):
    resource = httpx.get(f"{install.url}/.well-known/oauth-protected-resource/mcp", timeout=10)
    assert resource.status_code == 200
    assert resource.json()["resource"] == f"{install.url}/mcp"
    server = httpx.get(f"{install.url}/.well-known/oauth-authorization-server", timeout=10)
    assert server.json()["registration_endpoint"] == f"{install.url}/oauth/register"
    mcp = httpx.post(f"{install.url}/mcp", json={"jsonrpc": "2.0", "id": 1, "method": "ping"}, timeout=10)
    assert mcp.status_code == 401
    assert "resource_metadata=" in mcp.headers["www-authenticate"]
    project = "proj_00000000000000000000000000"
    portal = httpx.post(f"{install.url}/mcp/{project}", json={"jsonrpc": "2.0", "id": 1, "method": "ping"}, timeout=10)
    assert portal.status_code == 401
    assert f"/.well-known/oauth-protected-resource/mcp/{project}" in portal.headers["www-authenticate"]
    consent = httpx.get(f"{install.url}/authorize", timeout=10)
    assert consent.status_code == 200
    assert "text/html" in consent.headers["content-type"]


def test_the_gateway_reads_a_master_key_file_only_its_own_user_can_read(install):
    assert (install.work / "master-keys").stat().st_mode & 0o777 == 0o400
    assert (install.work / ".env").stat().st_mode & 0o777 == 0o600
    docker_reads_this_folder_as_you_from_a_vm = sys.platform == "darwin"
    if not docker_reads_this_folder_as_you_from_a_vm:
        assert (install.work / "master-keys").stat().st_uid == 65532
    assert install.health["gateway"] == "healthy"


def test_the_gateway_logs_the_address_caddy_saw_and_never_one_the_client_sends(install, owner):
    org = owner.get("/platform/v1/auth/session").json()["memberships"][0]["org_id"]
    project = next(p["id"] for p in owner.get(f"/platform/v1/orgs/{org}/projects").json()["data"] if p["name"] == "First project")
    base = f"/platform/v1/orgs/{org}/projects/{project}"
    key = owner.post(f"{base}/keys", json={"name": "forwarded"}).json()["secret"]
    forged = "203.0.113.7"
    oa = openai.OpenAI(base_url=f"{install.url}/v1", api_key=key, max_retries=0, default_headers={"X-Forwarded-For": forged})
    oa.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}])

    def logged() -> list[str] | None:
        rows = owner.get(f"{base}/logs").json().get("data", [])
        return [row["client_ip"] for row in rows] if len(rows) >= 3 else None

    addresses = poll(logged, "the third log entry", seconds=30)
    caddy = subprocess.run(
        ["docker", "inspect", "--format", f'{{{{(index .NetworkSettings.Networks "{PROJECT}_edge").IPAddress}}}}', f"{PROJECT}-caddy-1"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    assert caddy.startswith("172.30.83."), caddy
    assert forged not in addresses
    assert caddy not in addresses
    assert all(addresses), addresses


def test_wait_returns_once_every_service_is_healthy_caddy_included(install):
    shipped = ("caddy", "gateway", "console", "postgres", "clickhouse", "valkey")
    assert {name: install.health.get(name) for name in shipped} == dict.fromkeys(shipped, "healthy")


def test_two_gateway_replicas_share_the_edge_and_one_serves_while_the_other_is_down(install, owner):
    listed = install.run("ps", "--format", "json", "gateway", capture_output=True).stdout
    gateways = {s["Name"]: s["Health"] for s in map(json.loads, listed.splitlines())}
    assert gateways == {f"{PROJECT}-gateway-1": "healthy", f"{PROJECT}-gateway-2": "healthy"}
    probe = lambda: httpx.get(f"{install.url}/platform/v1/auth/methods", timeout=10).status_code
    assert probe() == 200
    subprocess.run(["docker", "stop", f"{PROJECT}-gateway-1"], check=True, capture_output=True)
    try:
        answered = [probe() for _ in range(10)]
        assert answered.count(200) >= 9, f"the other replica keeps serving: {answered}"
        session = owner.get("/platform/v1/auth/session")
        assert session.status_code == 200, "the session, kept in PostgreSQL, is valid on any replica"
    finally:
        subprocess.run(["docker", "start", f"{PROJECT}-gateway-1"], check=True, capture_output=True)
    poll(lambda: probe() == 200 or None, "the edge serving again", seconds=60)


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


def test_postgres_gives_a_quarter_of_its_memory_limit_to_shared_buffers(install):
    def show(setting: str) -> str:
        query = ["postgres", "psql", "-U", "kaeso", "-d", "kaeso", "-tAc", f"SHOW {setting}"]
        return install.run("exec", "-T", *query, capture_output=True).stdout.strip()

    assert show("shared_buffers") == "512MB"
    assert show("effective_cache_size") == "1536MB"


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


def as_runner(install: Install, *args: str) -> subprocess.CompletedProcess:
    work = str(install.work)
    return subprocess.run(
        ["docker", "run", "--rm", "--user", "0:0", "--cap-drop", "ALL", "--cap-add", "DAC_READ_SEARCH",
         "--security-opt", "no-new-privileges:true", "--read-only", "--tmpfs", "/tmp:rw,noexec,nosuid,size=16m",
         "--network", f"{PROJECT}_docker", "--network-alias", "updater-runner", "-v", f"{work}:{work}:ro", "-w", work,
         "-e", "DOCKER_HOST=tcp://docker-proxy:2375", "-e", f"COMPOSE_PROJECT_NAME={PROJECT}", "-e", "HOME=/tmp",
         install.env["KAESO_TEST_UPDATER_IMAGE"], "compose", *args],
        capture_output=True,
        text=True,
        timeout=600,
    )


def container_ids(install: Install, service: str) -> list[str]:
    return install.run("ps", "--all", "--quiet", service, capture_output=True).stdout.split()


def test_the_compose_runner_applies_a_changed_compose_file_through_the_proxy(install):
    compose = install.work / "compose.yaml"
    original = compose.read_text()
    marker = "    networks: [edge]\n\n  updater-init:"
    assert marker in original
    unchanged = ("gateway", "postgres", "clickhouse", "valkey", "caddy")
    before = {service: container_ids(install, service) for service in unchanged}
    try:
        compose.write_text(original.replace(marker, "    environment:\n      KAESO_RUNNER_TEST: applied\n" + marker))
        applied = as_runner(install, "up", "300")
        assert applied.returncode == 0, applied.stderr
        env = subprocess.run(
            ["docker", "inspect", "--format", "{{json .Config.Env}}", *container_ids(install, "console")],
            capture_output=True, text=True, check=True,
        ).stdout
        assert "KAESO_RUNNER_TEST=applied" in env
        assert {service: container_ids(install, service) for service in unchanged} == before, "an unchanged service is left as it is"
        assert as_runner(install, "replace", "120").returncode == 0
        unknown = as_runner(install, "down")
        assert unknown.returncode == 2, unknown.stderr
    finally:
        compose.write_text(original)
        install.run("up", "-d", "--wait", "--wait-timeout", "300")


def test_docker_proxy_refuses_the_runner_a_bind_mount_outside_the_install_folder(install):
    body = json.dumps({"Image": "sha256:" + "0" * 64, "HostConfig": {"Binds": ["/etc:/host:ro"]}})
    refused = subprocess.run(
        ["docker", "run", "--rm", "--network", f"{PROJECT}_docker", "--network-alias", "updater-runner",
         "--entrypoint", "wget", install.console, "-S", "-qO-", "-T", "5", "--header", "Content-Type: application/json",
         "--post-data", body, "http://docker-proxy:2375/v1.44/containers/create"],
        capture_output=True,
        text=True,
    )
    assert "403 Forbidden" in refused.stderr, refused.stderr
    version = subprocess.run(
        ["docker", "run", "--rm", "--network", f"{PROJECT}_docker", "--network-alias", "updater-runner",
         "--entrypoint", "wget", install.console, "-qO-", "-T", "5", "http://docker-proxy:2375/v1.44/version"],
        capture_output=True,
        text=True,
    )
    assert version.returncode == 0, "the runner's alias reaches the proxy, unlike any other container"


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
            portal = httpx.post(f"{url}/mcp/proj_00000000000000000000000000", json={}, timeout=10)
            assert portal.status_code == 503
            assert portal.json() == {"error": {"type": "unavailable", "message": message}}

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

            gemini = httpx.post(f"{url}/v1beta/models/gemini-3.5-flash:generateContent", json={}, timeout=10)
            assert gemini.status_code == 503
            assert gemini.json() == {"error": {"code": 503, "message": message, "status": "UNAVAILABLE"}}

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
