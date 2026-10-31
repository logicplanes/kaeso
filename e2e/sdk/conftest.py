import os
import re
import socket
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

import anthropic
import httpx
import openai
import pytest

ROOT = Path(__file__).resolve().parents[2]
BIN = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")) / "debug"
LOGS = ROOT / "target" / "e2e"
OPENAI_SECRET = "sk-test-openai-e2e-credential"
ANTHROPIC_SECRET = "sk-ant-test-e2e-credential"
SETUP_LINK = re.compile(r"#token=(\S+)")


@dataclass
class Gateway:
    url: str
    mock: str
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


def answers(url: str) -> bool:
    try:
        return httpx.get(url, timeout=1).status_code == 200
    except httpx.HTTPError:
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
    mock_url = f"http://127.0.0.1:{free_port()}"
    port = free_port()
    url = f"http://localhost:{port}"

    with open(LOGS / "mock.log", "w") as mock_log:
        mock = subprocess.Popen(
            [testkit, "mock-provider", mock_url.removeprefix("http://")],
            stdout=mock_log,
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
        KAESO_PROVIDERS_OPENAI_BASE_URL=mock_url,
        KAESO_PROVIDERS_ANTHROPIC_BASE_URL=mock_url,
        KAESO_STREAMS_KEEPALIVE_INTERVAL="1s",
    )
    log_path = LOGS / "gateway.log"
    with open(log_path, "w") as log:
        server = subprocess.Popen([kaeso, "serve"], env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        wait_until(lambda: answers(f"{url}/healthz"), f"the gateway answering (log: {log_path})")
        printed = subprocess.run([kaeso, "setup-link"], env=env, capture_output=True, text=True, check=True).stdout
        token = SETUP_LINK.search(printed).group(1)
        assert token not in log_path.read_text(), "the setup token reached the gateway's log"
        yield Gateway(url=url, mock=mock_url, setup_token=token)
    finally:
        stop(server)
        stop(mock)
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
        projects = self.http.get(f"/platform/v1/orgs/{self.org}/projects").json()
        self.project = projects["data"][0]["id"]
        self.base = f"/platform/v1/orgs/{self.org}/projects/{self.project}"
        for provider, secret in (("openai", OPENAI_SECRET), ("anthropic", ANTHROPIC_SECRET)):
            self.post(
                f"/platform/v1/orgs/{self.org}/credentials",
                {"provider": provider, "name": provider, "secret": secret},
            )
        self.models = {}
        for provider, model, overrides in (
            ("anthropic", "claude-haiku-4-5", None),
            ("anthropic", "claude-sonnet-4-6", None),
            ("openai", "gpt-4o", None),
            ("openai", "gpt-5-mini", None),
            ("openai", "text-embedding-3-small", None),
            ("openai", "gpt-4o-mini", {"timeout_seconds": 2}),
        ):
            body = {"provider": provider, "model_id": model}
            if overrides:
                body["overrides"] = overrides
            self.models[model] = self.post(f"{self.base}/models", body)["id"]
        self.post(
            f"{self.base}/aliases",
            {"name": "chat-default", "catalogue_model_id": self.models["claude-haiku-4-5"]},
        )
        self.key = self.post(f"{self.base}/keys", {"name": "e2e"})["secret"]
        self.narrow_key = self.post(
            f"{self.base}/keys", {"name": "narrow", "models": [self.models["gpt-4o"]]}
        )["secret"]
        self.platform_key = self.post(f"{self.base}/keys", {"name": "platform", "access": "platform"})[
            "secret"
        ]

    def post(self, path: str, body: dict) -> dict:
        r = self.http.post(path, json=body)
        assert r.status_code in (200, 201), r.text
        return r.json()


@pytest.fixture(scope="session")
def install(gateway: Gateway) -> Install:
    return Install(gateway)


@pytest.fixture
def oa(install: Install) -> openai.OpenAI:
    return openai.OpenAI(base_url=f"{install.url}/v1", api_key=install.key, max_retries=0)


@pytest.fixture
def an(install: Install) -> anthropic.Anthropic:
    return anthropic.Anthropic(base_url=install.url, api_key=install.key, max_retries=0)


class Mock:
    def __init__(self, url: str) -> None:
        self.http = httpx.Client(base_url=url, timeout=10)

    def reset(self) -> None:
        self.http.post("/__mock/reset").raise_for_status()

    def received(self) -> list:
        return self.http.get("/__mock/received").json()

    def last(self) -> dict:
        return self.received()[-1]

    def streams(self) -> dict:
        return self.http.get("/__mock/streams").json()


@pytest.fixture
def mock(gateway: Gateway) -> Mock:
    m = Mock(gateway.mock)
    m.reset()
    return m
