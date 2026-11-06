import os
import re
import socket
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

import httpx
import pytest

ROOT = Path(__file__).resolve().parents[2]
BIN = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug"
LOGS = ROOT / "target" / "e2e"
SERVER_TOKEN = "mcp-e2e-server-token"
SETUP_LINK = re.compile(r"#token=(\S+)")


@dataclass
class Gateway:
    url: str
    mcp: str
    setup_token: str


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def wait_until(check, what: str, seconds: float = 60) -> None:
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(0.1)
    raise TimeoutError(f"{what} did not happen within {seconds} seconds")


def ready(url: str) -> bool:
    try:
        return httpx.get(url, timeout=1).json()["status"] == "ready"
    except (httpx.HTTPError, ValueError):
        return False


def stop(process: subprocess.Popen) -> None:
    process.terminate()
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


@pytest.fixture(scope="session")
def gateway():
    subprocess.run(
        ["cargo", "build", "--quiet", "-p", "kaeso", "-p", "kaeso-testkit", "--bins"],
        cwd=ROOT,
        check=True,
    )
    LOGS.mkdir(parents=True, exist_ok=True)
    testkit, kaeso = str(BIN / "kaeso-testkit"), str(BIN / "kaeso")
    mcp_addr = f"127.0.0.1:{free_port()}"
    port = free_port()
    url = f"http://localhost:{port}"

    with open(LOGS / "mock-mcp.log", "w") as mcp_log:
        mcp = subprocess.Popen(
            [testkit, "mock-mcp", mcp_addr, SERVER_TOKEN],
            stdout=mcp_log,
            stderr=subprocess.STDOUT,
        )
    services = subprocess.run(
        [testkit, "fresh-services", "--temporary"], capture_output=True, text=True, check=True
    ).stdout
    env = dict(os.environ)
    env.update(line.split("=", 1) for line in services.splitlines() if "=" in line)
    database = env["KAESO_CLICKHOUSE_DATABASE"]
    env.update(
        KAESO_PUBLIC_URL=url,
        KAESO_LISTEN=f"127.0.0.1:{port}",
        KAESO_METRICS_LISTEN=f"127.0.0.1:{free_port()}",
        KAESO_MASTER_KEYS=subprocess.check_output([kaeso, "master-key", "generate"], text=True).strip(),
        KAESO_MAIL_TRANSPORT="log",
        KAESO_KPMR_SYNC="false",
        KAESO_LOG_FORMAT="json",
        KAESO_MCP_SERVERS_ALLOWED_NETWORKS="127.0.0.0/8",
    )
    log_path = LOGS / "gateway-mcp.log"
    with open(log_path, "w") as log:
        server = subprocess.Popen([kaeso, "serve"], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        wait_until(lambda: ready(f"{url}/readyz"), f"the gateway being ready (log: {log_path})")
        printed = subprocess.run([kaeso, "setup-link"], env=env, capture_output=True, text=True, check=True).stdout
        token = SETUP_LINK.search(printed).group(1)
        yield Gateway(url=url, mcp=f"http://{mcp_addr}/mcp", setup_token=token)
        assert SERVER_TOKEN not in log_path.read_text(), "the server's secret header reached the gateway's log"
    finally:
        stop(server)
        stop(mcp)
        subprocess.run([testkit, "drop-services", database], check=False)


class Install:
    def __init__(self, gateway: Gateway) -> None:
        self.url = gateway.url
        self.http = httpx.Client(base_url=gateway.url, headers={"origin": gateway.url}, timeout=30)
        self.post(
            "/platform/v1/auth/setup",
            {
                "token": gateway.setup_token,
                "email": "owner@example.com",
                "name": "Owner",
                "password": "correct horse battery",
                "org_name": "E2E",
            },
        )
        session = self.http.get("/platform/v1/auth/session").json()
        self.org = session["memberships"][0]["org_id"]
        self.project = self.post(f"/platform/v1/orgs/{self.org}/projects", {"name": "E2E"})["id"]
        self.base = f"/platform/v1/orgs/{self.org}/projects/{self.project}"
        servers = f"/platform/v1/orgs/{self.org}/mcp-servers"
        server = self.post(
            servers,
            {
                "handle": "x",
                "name": "X",
                "url": gateway.mcp,
                "auth_method": "header",
                "projects": [self.project],
            },
        )["id"]
        self.post(
            f"{servers}/{server}/connection",
            {"header": {"name": "Authorization", "value": f"Bearer {SERVER_TOKEN}"}},
        )
        wait_until(lambda: len(self.tools()) == 3, "the server's tools arriving in the project")
        ids = {t["name"]: t["id"] for t in self.tools()}
        on = self.http.patch(
            f"{self.base}/tools",
            json={
                "tools": [
                    {"id": ids["create_post"], "enabled": True},
                    {"id": ids["search_posts"], "enabled": True},
                ]
            },
        )
        assert on.status_code == 200, on.text
        key = self.post(f"{self.base}/keys", {"name": "agent", "tools": True})
        self.key, self.key_id = key["secret"], key["id"]
        self.plain_key = self.post(f"{self.base}/keys", {"name": "models only"})["secret"]
        self.portal = f"{gateway.url}/mcp/{self.project}"

    def tools(self) -> list:
        r = self.http.get(f"{self.base}/tools")
        assert r.status_code == 200, r.text
        return r.json()["data"]

    def post(self, path: str, body: dict) -> dict:
        r = self.http.post(path, json=body)
        assert r.status_code in (200, 201), r.text
        return r.json()

    def calls(self) -> list:
        r = self.http.get(f"{self.base}/logs/tool-calls", params={"key": self.key_id})
        assert r.status_code == 200, r.text
        return r.json()["data"]


@pytest.fixture(scope="session")
def install(gateway: Gateway) -> Install:
    return Install(gateway)
