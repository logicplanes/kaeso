from __future__ import annotations

import json

import httpx
import pytest
from typesafe_sdk import Image, RetryPolicy, TypeSafeAPIError, TypeSafeClient

from conftest import FIXTURES, expected

SCENARIO_KINDS = ("decide", "error")
SCENARIO_SOURCES = ("system_one",)
SOURCE = FIXTURES / "system_one"
NOT_SENT = ("model", "state", "questions", "assets")


def client(base_url: str) -> TypeSafeClient:
    return TypeSafeClient(api_key="test", base_url=base_url, compression=False, retry=RetryPolicy(max_retries=0))


def fixture_request(name: str) -> tuple[dict, dict]:
    body = json.loads((SOURCE / "requests" / f"{name}.json").read_text())
    headers = SOURCE / "requests" / f"{name}.headers.json"
    return body, json.loads(headers.read_text()) if headers.exists() else {}


def close(a: float, b: float) -> bool:
    return a == pytest.approx(b, abs=1e-9)


def check_answer(question: dict, got, want: dict) -> None:
    if want["type"] == "predicate":
        assert got.type == "noul"
        assert close(got.noul, want["probability"])
    elif want["type"] == "choice":
        options = list(question["criteria"])
        assert got.choice == options[want["option"]]
        for option, p in zip(options, want["probabilities"]):
            if p is not None:
                assert close(got.probabilities[option], p)
        assert close(got.confidence, want["confidence"])
    elif want["type"] == "score":
        assert close(got.score, want["score"])
        for level, p in enumerate(want["probabilities"]):
            if p is not None:
                assert close(got.probabilities[level], p)
        assert close(got.confidence, want["confidence"])
        assert len(got.legend) == len(question["criteria"])
    elif want["type"] == "box":
        assert (got.box.top, got.box.bottom, got.box.left, got.box.right) == (
            want["top"],
            want["bottom"],
            want["left"],
            want["right"],
        )
        assert got.confidence == want["confidence"]
    else:
        raise AssertionError(want)


def test_system_one(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    with client(base_url) as c:
        if scenario["kind"] == "error":
            with pytest.raises(TypeSafeAPIError) as info:
                c.system_one("hi", {"q": {"type": "noul", "instructions": "Hi?"}}, model=model)
            assert info.value.status == want["status"], want
            assert isinstance(info.value.body, dict) and "detail" in info.value.body
            if want["status"] == 429:
                assert info.value.headers.get("retry-after")
            return
        req, headers = fixture_request(scenario["name"])
        assets = {ref: Image.from_base64(a["data_base64"]) for ref, a in req.get("assets", {}).items()}
        extra = {k: v for k, v in req.items() if k not in NOT_SENT}
        got = c.system_one(
            req["state"],
            req["questions"],
            model=model,
            assets=assets or None,
            extra_headers=headers or None,
            extra_body=extra or None,
        )
    assert got.model == want["model"]
    assert (got.usage.input_tokens, got.usage.output_tokens) == (want["input_tokens"], want["output_tokens"])
    assert list(got.answers) == list(req["questions"])
    for (key, question), answer in zip(req["questions"].items(), want["answers"]):
        check_answer(question, got.answers[key], answer)


def test_openai_refusals_reach_system_one_apps(base_url):
    want = expected(base_url, {"source": "openai_decisions", "name": "refusal", "kind": "decide", "stream": False})
    req = json.loads((FIXTURES / "openai_decisions" / "requests" / "refusal.json").read_text())
    how_to, topic = req["questions"]
    questions = {
        "how_to": {"type": "noul", "instructions": how_to["instructions"]},
        "topic": {
            "type": "choice",
            "instructions": topic["instructions"],
            "criteria": {c["value"]: None for c in topic["choices"]},
        },
    }
    body = {"model": "openai_decisions:refusal", "state": req["input"], "questions": questions}
    raw = httpx.post(f"{base_url}/v1/systemone", json=body, headers={"authorization": "Bearer test"}).json()
    assert raw["answers"]["how_to"] == {"type": "refusal"}
    with client(base_url) as c:
        got = c.system_one(req["input"], questions, model="openai_decisions:refusal")
    assert "how_to" not in got.answers
    check_answer(questions["topic"], got.answers["topic"], want["answers"][1])
