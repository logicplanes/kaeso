import asyncio

import httpx2
import pytest
from mcp import Client
from mcp.client.streamable_http import streamable_http_client
from mcp.shared.exceptions import MCPError

from conftest import Install, wait_until

MODERN = "2026-07-28"
VERSIONS = [("auto", MODERN), ("legacy", "2025-11-25")]


class Portal:
    def __init__(self, install: Install, mode: str, key: str | None = None) -> None:
        self.statuses: list[int] = []

        async def seen(response: httpx2.Response) -> None:
            self.statuses.append(response.status_code)

        http = httpx2.AsyncClient(
            headers={"Authorization": f"Bearer {key or install.key}"},
            event_hooks={"response": [seen]},
        )
        self.client = Client(streamable_http_client(install.portal, http_client=http), mode=mode)

    def run(self, steps):
        async def session():
            async with self.client as c:
                try:
                    return await steps(c)
                except MCPError as e:
                    return e

        return asyncio.run(session())


async def names(c: Client) -> tuple[str, list[str]]:
    tools = await c.list_tools()
    return c.protocol_version, [t.name for t in tools.tools]


def call(name: str, arguments: dict):
    async def steps(c: Client):
        await c.list_tools()
        return await c.call_tool(name, arguments)

    return steps


@pytest.mark.parametrize(("mode", "version"), VERSIONS)
def test_the_sdk_lists_the_tools_the_project_turned_on(install: Install, mode: str, version: str) -> None:
    assert Portal(install, mode).run(names) == (version, ["x__create_post", "x__search_posts"])


@pytest.mark.parametrize(("mode", "version"), VERSIONS)
def test_the_sdk_calls_a_tool_and_the_call_is_logged(install: Install, mode: str, version: str) -> None:
    text = f"Hello from {version}"
    result = Portal(install, mode).run(call("x__create_post", {"text": text}))
    assert result.is_error is False
    assert result.content[0].text == f'create_post ran with {{"text":"{text}"}}'

    def logged() -> bool:
        return any(
            c["tool"] == "x__create_post" and c["status"] == "ok" and c["protocol_version"] == version
            for c in install.calls()
        )

    wait_until(logged, "the call reaching the request logs", seconds=30)


@pytest.mark.parametrize("mode", ["auto", "legacy"])
def test_a_tool_the_project_left_off_is_refused(install: Install, mode: str) -> None:
    refused = Portal(install, mode).run(call("x__get_me", {}))
    assert isinstance(refused, MCPError)
    assert (refused.code, refused.error.message) == (
        -32602,
        "The tool `x__get_me` is off in this project; a project admin turns it on under Tools.",
    )


def test_a_key_without_tool_access_is_turned_away(install: Install) -> None:
    portal = Portal(install, "auto", install.plain_key)
    with pytest.raises(Exception):
        portal.run(names)
    assert set(portal.statuses) == {403}


@pytest.mark.parametrize(("mode", "version"), VERSIONS)
def test_the_sdk_reads_a_servers_view_through_the_portal(install: Install, mode: str, version: str) -> None:
    ids = {t["name"]: t["id"] for t in install.tools()}

    def switch(on: bool) -> None:
        r = install.http.patch(f"{install.base}/tools", json={"tools": [{"id": ids["get_me"], "enabled": on}]})
        assert r.status_code == 200, r.text

    async def steps(c: Client):
        tools = await c.list_tools()
        resources = await c.list_resources()
        read = await c.read_resource("ui://x/x/me")
        return tools, resources, read

    switch(True)
    try:
        tools, resources, read = Portal(install, mode).run(steps)
    finally:
        switch(False)
    me = next(t for t in tools.tools if t.name == "x__get_me")
    assert me.meta == {"ui": {"resourceUri": "ui://x/x/me"}}
    assert [(r.uri, r.mime_type) for r in resources.resources] == [("ui://x/x/me", "text/html;profile=mcp-app")]
    item = read.contents[0]
    assert (item.uri, item.mime_type) == ("ui://x/x/me", "text/html;profile=mcp-app")
    assert "Signed in to X" in item.text
    assert item.meta["ui"]["csp"] == {"connectDomains": ["https://api.x.example"]}
