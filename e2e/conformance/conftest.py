from __future__ import annotations

import json
import os
import pathlib
import socket
import subprocess
import time
import urllib.parse
import urllib.request

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "crates" / "kaeso-codecs" / "fixtures"
TARGET = pathlib.Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))


def _free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _wait(url: str, proc: subprocess.Popen | None) -> None:
    deadline = time.time() + 600
    while time.time() < deadline:
        if proc is not None and proc.poll() is not None:
            raise RuntimeError("conformance server exited early")
        try:
            urllib.request.urlopen(url + "/v1/models", timeout=2).read()
            return
        except OSError:
            time.sleep(0.5)
    raise RuntimeError("conformance server did not start")


@pytest.fixture(scope="session")
def base_url():
    url = os.environ.get("KPP_CONFORMANCE_URL")
    if url:
        _wait(url, None)
        yield url
        return
    port = _free_port()
    env = dict(os.environ, KPP_CONFORMANCE_PORT=str(port))
    subprocess.run(["cargo", "build", "--release", "--quiet", "-p", "kpp-conformance"], cwd=ROOT, check=True)
    binary = TARGET / "release" / "kpp-conformance"
    proc = subprocess.Popen([str(binary)], env=env)
    url = f"http://127.0.0.1:{port}"
    try:
        _wait(url, proc)
        yield url
    finally:
        proc.terminate()
        proc.wait(timeout=10)


def fetch_json(url: str):
    with urllib.request.urlopen(url, timeout=30) as r:
        return json.load(r)


def expected(url: str, s: dict):
    query = urllib.parse.urlencode(
        {"source": s["source"], "name": s["name"], "kind": s["kind"], "stream": "true" if s["stream"] else "false"}
    )
    return fetch_json(f"{url}/_expected?{query}")


GENERATION = ("anthropic_messages", "openai_chat", "openai_responses", "gemini")

# Video fixtures of providers that keep them beside other fixtures, by the folder they are in.
VIDEO_PREFIXES = {"openai_videos": ("", "providers/togetherai/"), "elevenlabs": ("video/",)}

FOLDERS = {
    "generate": [("responses", False), ("streams", True)],
    "count_tokens": [("responses/count_tokens", False), ("responses/input_tokens", False)],
    "embed": [("responses", False)],
    "image": [("responses", False), ("streams", True)],
    "decide": [("responses", False)],
    "error": [("errors", False)],
    "video": [("responses", False), ("polls", False)],
}


def _scenarios(source: str, kind: str) -> list[dict]:
    out = []
    prefixes = VIDEO_PREFIXES.get(source, ("",)) if kind == "video" else ("",)
    for prefix in prefixes:
        for folder, stream in FOLDERS[kind]:
            d = FIXTURES / source / (prefix + folder)
            if not d.is_dir():
                continue
            for f in sorted(d.iterdir()):
                if f.suffix not in (".json", ".sse") or f.name.endswith(".meta.json"):
                    continue
                name = f.stem
                if kind == "video":
                    name = f"{prefix}{folder}/{name}"
                elif folder.startswith("responses/"):
                    name = folder.split("/", 1)[1] + "/" + name
                if kind == "error":
                    name = "errors/" + name
                out.append({"source": source, "name": name, "kind": kind, "stream": stream})
    return out


def pytest_generate_tests(metafunc):
    if "scenario" not in metafunc.fixturenames:
        return
    wanted = getattr(metafunc.module, "SCENARIO_KINDS", ("generate",))
    sources = getattr(metafunc.module, "SCENARIO_SOURCES", GENERATION)
    out = [s for source in sources for kind in wanted for s in _scenarios(source, kind)]
    metafunc.parametrize("scenario", out, ids=[f"{s['source']}:{s['name']}{'+stream' if s['stream'] else ''}" for s in out])
