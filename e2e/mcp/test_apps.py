import asyncio
import base64
import hashlib
import secrets
from urllib.parse import parse_qs, urlparse

import httpx
import httpx2
from mcp import Client
from mcp.client.streamable_http import streamable_http_client

from conftest import Install

REDIRECT = "http://127.0.0.1:50000/callback"
VIEW = "ui://kaeso/stats"


def agent_token(install: Install) -> str:
    http = httpx.Client(base_url=install.url, timeout=30)
    registered = http.post(
        "/oauth/register",
        json={"client_name": "Claude", "redirect_uris": [REDIRECT], "token_endpoint_auth_method": "none"},
    )
    assert registered.status_code == 201, registered.text
    client = registered.json()["client_id"]
    verifier = secrets.token_urlsafe(48)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
    asked = http.get(
        "/oauth/authorize",
        params={
            "response_type": "code",
            "client_id": client,
            "redirect_uri": REDIRECT,
            "state": "e2e",
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            "resource": f"{install.url}/mcp",
            "scope": "kaeso",
        },
    )
    assert asked.status_code == 303, asked.text
    request = asked.headers["location"].split("#request=", 1)[1]
    grant = {
        "preset": "builder",
        "customized": False,
        "instance": [],
        "account": ["account.view"],
        "orgs": [
            {
                "org": install.org,
                "included": True,
                "rights": ["org.view", "usage.org.view", "costs.org.view"],
                "projects": {"all": True, "rights": ["project.view", "usage.view", "costs.view"], "selected": []},
            }
        ],
        "future_orgs": None,
    }
    approved = install.post("/platform/v1/me/agent-requests/approve", {"request": request, "name": "Usage view", "grant": grant})
    code = parse_qs(urlparse(approved["redirect_to"]).query)["code"][0]
    tokens = http.post(
        "/oauth/token",
        data={"grant_type": "authorization_code", "code": code, "client_id": client, "redirect_uri": REDIRECT, "code_verifier": verifier},
    )
    assert tokens.status_code == 200, tokens.text
    return tokens.json()["access_token"]


def run(install: Install, steps):
    http = httpx2.AsyncClient(headers={"Authorization": f"Bearer {agent_token(install)}"})

    async def session():
        async with Client(streamable_http_client(f"{install.url}/mcp", http_client=http), mode="legacy") as c:
            return await steps(c)

    return asyncio.run(session())


def test_the_sdk_finds_the_usage_view_through_the_tool_and_reads_it(install: Install) -> None:
    async def steps(c: Client):
        tools = await c.list_tools()
        resources = await c.list_resources()
        read = await c.read_resource(VIEW)
        return tools, resources, read

    tools, resources, read = run(install, steps)
    stats = next(t for t in tools.tools if t.name == "show_usage_stats")
    assert stats.meta == {"ui": {"resourceUri": VIEW}, "ui/resourceUri": VIEW}
    assert [(r.uri, r.mime_type) for r in resources.resources] == [(VIEW, "text/html;profile=mcp-app")]
    item = read.contents[0]
    assert item.mime_type == "text/html;profile=mcp-app"
    assert item.text.startswith("<!doctype html>")
    assert item.meta["ui"]["csp"] == {"connectDomains": [], "resourceDomains": [], "frameDomains": [], "baseUriDomains": []}


def test_the_usage_tool_answers_with_text_and_the_data_the_view_draws(install: Install) -> None:
    async def steps(c: Client):
        return await c.call_tool("show_usage_stats", {"metric": "requests", "range": "7d", "group_by": "provider", "agent_model": "claude"})

    result = run(install, steps)
    assert result.is_error is False
    assert result.content[0].text.startswith("Requests of E2E in the last 7 days: 0.")
    data = result.structured_content
    assert (data["metric"], data["range"], data["group_by"], data["step"]) == ("requests", "7d", "provider", "day")
    assert len(data["series"]) == 7
    assert data["scope"]["org"] == {"id": install.org, "name": "E2E"}
