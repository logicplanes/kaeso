import json
import os
import shutil
import subprocess
from pathlib import Path

import anthropic
import httpx
import openai
import pytest

from test_install import ROOT, SETUP_LINK, UNREACHABLE, Install, built, free_port, image, poll

PROJECT = "kaeso-edge-test"


@pytest.fixture(scope="module")
def edge(tmp_path_factory):
    work = tmp_path_factory.mktemp("edge")
    shutil.copy(ROOT / "deploy" / "compose.yaml", work / "compose.yaml")
    shutil.copy(Path(__file__).with_name("compose.edge.yaml"), work / "compose.edge.yaml")
    port = free_port()
    gateway = image("KAESO_TEST_GATEWAY_IMAGE", "gateway")
    mock = image("KAESO_TEST_MOCK_IMAGE", "mock")
    console = built("KAESO_TEST_CONSOLE_IMAGE", "console")
    (work / "master-keys").write_text(
        subprocess.check_output(["docker", "run", "--rm", gateway, "master-key", "generate"], text=True)
    )
    subprocess.run(
        ["docker", "run", "--rm", "--user", "0:0", "-v", f"{work}:/kaeso", "--entrypoint", "sh", console,
         "-c", "chown 65532:65532 /kaeso/master-keys && chmod 0400 /kaeso/master-keys"],
        check=True,
    )
    env = {
        "KAESO_DOMAIN": "kaeso.example.com",
        "POSTGRES_PASSWORD": os.urandom(16).hex(),
        "POSTGRES_SUPERUSER_PASSWORD": os.urandom(16).hex(),
        "CLICKHOUSE_PASSWORD": os.urandom(16).hex(),
        "VALKEY_PASSWORD": os.urandom(16).hex(),
        "KAESO_TEST_PORT": str(port),
        "KAESO_TEST_GATEWAY_IMAGE": gateway,
        "KAESO_TEST_MOCK_IMAGE": mock,
        "KAESO_TEST_CONSOLE_IMAGE": console,
        "KAESO_DIR": str(work),
        "EDGE_SUBNET": "10.93.83.0/24",
    }
    (work / ".env").touch(mode=0o600)
    (work / ".env").write_text("".join(f"{name}={value}\n" for name, value in env.items()))
    compose = ["docker", "compose", "-p", PROJECT, "-f", "compose.yaml", "-f", "compose.edge.yaml"]

    def run(*args: str, check: bool = True, **kwargs) -> subprocess.CompletedProcess:
        return subprocess.run([*compose, *args], cwd=work, check=check, text=True, **kwargs)

    url = f"http://localhost:{port}"
    try:
        run("up", "-d", "--wait", "--wait-timeout", "300")
        listed = run("ps", "--all", "--format", "json", capture_output=True).stdout
        health = {s["Service"]: s["Health"] for s in map(json.loads, listed.splitlines())}
        printed = run("exec", "-T", "gateway", "kaeso", "setup-link", capture_output=True).stdout
        token = SETUP_LINK.search(printed).group(1)
        yield Install(url, token, env, run, health, work, console)
    finally:
        subprocess.run([*compose, "logs", "--no-color"], cwd=work, stdout=open(work / "compose.log", "w"))
        subprocess.run([*compose, "down", "-v", "--remove-orphans"], cwd=work, check=False)


@pytest.fixture(scope="module")
def owner(edge) -> httpx.Client:
    http = httpx.Client(base_url=edge.url, headers={"origin": edge.url}, timeout=30)
    r = http.post(
        "/platform/v1/auth/setup",
        json={"token": edge.token, "email": "owner@example.com", "name": "Owner", "password": "correct horse battery", "org_name": "First org"},
    )
    assert r.status_code == 200, r.text
    return http


@pytest.fixture(scope="module")
def project(owner) -> tuple[str, str]:
    org = owner.get("/platform/v1/auth/session").json()["memberships"][0]["org_id"]
    project = owner.post(f"/platform/v1/orgs/{org}/projects", json={"name": "Edge project"}).json()["id"]
    base = f"/platform/v1/orgs/{org}/projects/{project}"
    for provider, secret in [("openai", "sk-test-edge-openai"), ("anthropic", "sk-ant-test-edge")]:
        assert owner.post(f"/platform/v1/orgs/{org}/credentials", json={"provider": provider, "name": provider, "secret": secret}).status_code == 201
    for provider, model in [("openai", "gpt-4o"), ("anthropic", "claude-haiku-4-5")]:
        assert owner.post(f"{base}/models", json={"provider": provider, "model_id": model}).status_code == 201
    key = owner.post(f"{base}/keys", json={"name": "edge"}).json()["secret"]
    return base, key


def test_the_edge_is_healthy_without_caddy_and_the_updater(edge):
    assert edge.health["edge"] == "healthy"
    assert "caddy" not in edge.health
    assert "updater" not in edge.health
    ok = httpx.get(f"{edge.url}/kaeso-edge/healthz", timeout=10)
    assert ok.status_code == 200 and ok.text == "ok"
    assert "server" not in ok.headers


def test_api_paths_reach_the_gateway_and_everything_else_the_console(edge):
    methods = httpx.get(f"{edge.url}/platform/v1/auth/methods", timeout=10)
    assert methods.status_code == 200 and "password" in methods.json()
    resource = httpx.get(f"{edge.url}/.well-known/oauth-protected-resource/mcp", timeout=10)
    assert resource.json()["resource"] == f"{edge.url}/mcp"
    mcp = httpx.post(f"{edge.url}/mcp", json={"jsonrpc": "2.0", "id": 1, "method": "ping"}, timeout=10)
    assert mcp.status_code == 401 and "resource_metadata=" in mcp.headers["www-authenticate"]
    portal = httpx.post(f"{edge.url}/mcp/proj_00000000000000000000000000", json={}, timeout=10)
    assert portal.status_code == 401
    for path in ["/", "/sign-in", "/settings/instance/updates", "/authorize"]:
        page = httpx.get(f"{edge.url}{path}", timeout=10)
        assert page.status_code == 200, path
        assert "text/html" in page.headers["content-type"], path
    assert httpx.get(f"{edge.url}/v1/models", timeout=10).status_code == 401
    voices = httpx.get(f"{edge.url}/v2/voices", timeout=10)
    assert voices.status_code == 401 and "detail" in voices.json(), "ElevenLabs' /v2 reaches the gateway"


def test_streams_pass_through_the_edge_chunk_by_chunk(edge, project):
    base, key = project
    oa = openai.OpenAI(base_url=f"{edge.url}/v1", api_key=key, max_retries=0)
    chunks = [c for c in oa.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}], stream=True)]
    assert len(chunks) > 1
    assert "".join(c.choices[0].delta.content or "" for c in chunks if c.choices)
    an = anthropic.Anthropic(base_url=edge.url, api_key=key, max_retries=0)
    with an.messages.stream(model="claude-haiku-4-5", max_tokens=50, messages=[{"role": "user", "content": "Hello"}]) as stream:
        assert any(stream.text_stream)
    with httpx.stream("POST", f"{edge.url}/v1/chat/completions", json={"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello"}], "stream": True}, headers={"authorization": f"Bearer {key}"}, timeout=30) as r:
        assert r.headers["content-type"].startswith("text/event-stream")
        assert "content-length" not in r.headers
        assert any(line.startswith("data: ") for line in r.iter_lines())


def test_a_forged_forwarded_address_is_dropped_by_the_edge(edge, owner, project):
    base, key = project
    forged = "203.0.113.7"
    oa = openai.OpenAI(base_url=f"{edge.url}/v1", api_key=key, max_retries=0, default_headers={"X-Forwarded-For": forged})
    oa.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}])

    def logged() -> list[str] | None:
        rows = owner.get(f"{base}/logs").json().get("data", [])
        return [row["client_ip"] for row in rows] if len(rows) >= 3 else None

    addresses = poll(logged, "the log entries", seconds=60)
    edge_address = subprocess.run(
        ["docker", "inspect", "--format", f'{{{{(index .NetworkSettings.Networks "{PROJECT}_edge").IPAddress}}}}', f"{PROJECT}-edge-1"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    assert forged not in addresses
    assert edge_address not in addresses
    assert all(addresses), addresses


def test_the_edge_answers_every_api_in_its_format_while_the_gateway_is_away(edge):
    url = edge.url
    edge.run("stop", "gateway")
    try:
        platform = httpx.get(f"{url}/platform/v1/auth/session", timeout=10)
        assert platform.status_code == 503 and platform.headers["retry-after"] == "30"
        assert platform.json() == {"error": {"type": "unavailable", "message": UNREACHABLE}}
        oa = openai.OpenAI(base_url=f"{url}/v1", api_key="sk-kso-test", max_retries=0)
        with pytest.raises(openai.APIStatusError) as raised:
            oa.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}])
        assert raised.value.status_code == 503 and raised.value.body["message"] == UNREACHABLE
        an = anthropic.Anthropic(base_url=url, api_key="sk-kso-test", max_retries=0)
        with pytest.raises(anthropic.APIStatusError) as raised:
            an.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=[{"role": "user", "content": "Hello"}])
        assert raised.value.status_code == 529
        assert raised.value.body["error"] == {"type": "overloaded_error", "message": UNREACHABLE}
        gemini = httpx.post(f"{url}/v1beta/models/gemini-3.5-flash:generateContent", json={}, timeout=10)
        assert gemini.status_code == 503
        assert gemini.json() == {"error": {"code": 503, "message": UNREACHABLE, "status": "UNAVAILABLE"}}
        voices = httpx.get(f"{url}/v2/voices", headers={"xi-api-key": "sk-kso-test"}, timeout=10)
        assert voices.status_code == 503
        assert voices.json()["detail"]["message"] == UNREACHABLE
        page = httpx.get(f"{url}/", timeout=10)
        assert page.status_code == 200, "the console stays up while the gateway is away"
    finally:
        edge.run("start", "gateway")
        poll(lambda: httpx.get(f"{url}/platform/v1/auth/methods", timeout=5).status_code == 200 or None, "the gateway back", seconds=120)
