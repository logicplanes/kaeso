from __future__ import annotations

import json

import httpx
import pytest
from google import genai
from google.genai import types
from google.genai._interactions import APIStatusError

from conftest import FIXTURES, expected

SCENARIO_KINDS = ("decide", "error")
SCENARIO_SOURCES = ("gemini_decisions",)
SOURCE = FIXTURES / "gemini_decisions"
SENT = ("model", "input", "questions")


def client(base_url: str) -> genai.Client:
    return genai.Client(
        api_key="test",
        http_options=types.HttpOptions(
            base_url=base_url,
            api_version="v1beta",
            retry_options=types.HttpRetryOptions(attempts=1),
        ),
    )


def close(a: float, b: float) -> bool:
    return a == pytest.approx(b, abs=1e-9)


def check_answer(question: dict, got, want: dict, recomputed: bool) -> None:
    if want["type"] == "predicate":
        assert got.type == "noul"
        assert close(got.noul, want["probability"])
        confidence = max(want["probability"], 1 - want["probability"]) if recomputed else want["confidence"]
    elif want["type"] == "choice":
        options = list(question["criteria"])
        assert got.type == "choice"
        assert got.choice == options[want["option"]]
        wanted = {o: p for o, p in zip(options, want["probabilities"]) if p is not None}
        assert got.probabilities.keys() == wanted.keys()
        for option, p in wanted.items():
            assert close(got.probabilities[option], p)
        confidence = max(wanted.values()) if recomputed else want["confidence"]
    elif want["type"] == "score":
        assert got.type == "score"
        assert close(got.score, want["score"])
        wanted = {str(i): p for i, p in enumerate(want["probabilities"]) if p is not None}
        assert got.probabilities.keys() == wanted.keys()
        for level, p in wanted.items():
            assert close(got.probabilities[level], p)
        assert got.legend == {str(i): text for i, text in enumerate(question["criteria"])}
        confidence = max(wanted.values()) if recomputed else want["confidence"]
    else:
        raise AssertionError(want)
    assert close(got.confidence, confidence)


def test_gemini_decisions(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    if scenario["kind"] == "error":
        with pytest.raises(APIStatusError) as info:
            c.decisions.create(model=model, input="hi", questions={"q": {"type": "noul", "instructions": "Hi?"}})
        assert info.value.status_code == want["status"], want
        assert "error" in info.value.response.json()
        return
    req = json.loads((SOURCE / "requests" / f"{scenario['name']}.json").read_text())
    extra = {k: v for k, v in req.items() if k not in SENT}
    if "error" in want:
        with pytest.raises(APIStatusError) as info:
            c.decisions.create(model=model, input=req["input"], questions=req["questions"])
        assert info.value.status_code == 400
        assert info.value.response.json()["error"]["code"] == "safety"
        return
    got = c.decisions.create(model=model, input=req["input"], questions=req["questions"], extra_body=extra or None)
    assert got.usage.total_input_tokens == want["input_tokens"]
    assert got.usage.total_output_tokens == want["output_tokens"]
    assert list(got.answers) == list(req["questions"])
    for (key, question), answer in zip(req["questions"].items(), want["answers"]):
        check_answer(question, got.answers[key], answer, recomputed=False)


def system_one_as_gemini(name: str) -> dict:
    body = json.loads((FIXTURES / "system_one" / "requests" / f"{name}.json").read_text())
    questions = {}
    for key, q in body["questions"].items():
        out = {"type": q["type"], "instructions": q["instructions"]}
        if q["type"] == "choice":
            out["criteria"] = {k: v or "" for k, v in q["criteria"].items()}
        if q["type"] == "score":
            out["criteria"] = q["criteria"]
        questions[key] = out
    return {"input": body["state"], "questions": questions}


@pytest.mark.parametrize("name", ["noul", "choice", "score"])
def test_system_one_answers_reach_gemini_apps(base_url, name):
    want = expected(base_url, {"source": "system_one", "name": name, "kind": "decide", "stream": False})
    req = system_one_as_gemini(name)
    c = client(base_url)
    got = c.decisions.create(model=f"system_one:{name}", **req)
    assert got.usage.total_input_tokens == want["input_tokens"]
    assert got.usage.total_tokens == want["input_tokens"] + want["output_tokens"]
    for (key, question), answer in zip(req["questions"].items(), want["answers"]):
        check_answer(question, got.answers[key], answer, recomputed=True)


def openai_as_gemini(name: str) -> dict:
    body = json.loads((FIXTURES / "openai_decisions" / "requests" / f"{name}.json").read_text())
    questions = {}
    for q in body["questions"]:
        out = {"type": {"predicate": "noul"}.get(q["type"], q["type"]), "instructions": q["instructions"]}
        if q["type"] == "choice":
            out["criteria"] = {c["value"]: c.get("description", "") for c in q["choices"]}
        if q["type"] == "score":
            out["criteria"] = [f"{level['label']}: {level['description']}" for level in q["levels"]]
        questions[q["name"]] = out
    return {"input": body["input"], "questions": questions}


@pytest.mark.parametrize("name", ["predicate", "choice", "score"])
def test_openai_answers_reach_gemini_apps(base_url, name):
    want = expected(base_url, {"source": "openai_decisions", "name": name, "kind": "decide", "stream": False})
    req = openai_as_gemini(name)
    c = client(base_url)
    got = c.decisions.create(model=f"openai_decisions:{name}", **req)
    assert got.usage.total_input_tokens == want["input_tokens"]
    for (key, question), answer in zip(req["questions"].items(), want["answers"]):
        check_answer(question, got.answers[key], answer, recomputed=True)


def test_openai_refusals_reach_gemini_apps_as_a_safety_error(base_url):
    req = openai_as_gemini("refusal")
    c = client(base_url)
    with pytest.raises(APIStatusError) as info:
        c.decisions.create(model="openai_decisions:refusal", **req)
    assert info.value.status_code == 400
    error = info.value.response.json()["error"]
    assert error["code"] == "safety"
    assert "`how_to`" in error["message"]
    raw = httpx.post(
        f"{base_url}/v1beta/decisions",
        json={"model": "openai_decisions:refusal", **req},
        headers={"x-goog-api-key": "test"},
    )
    assert raw.status_code == 400


def test_gemini_safety_blocks_reach_openai_and_system_one_apps(base_url):
    body = {"model": "gemini_decisions:blocked", "input": "x", "questions": [{"type": "predicate", "name": "is_spam", "instructions": "Spam?"}]}
    r = httpx.post(f"{base_url}/v1/decisions", json=body, headers={"authorization": "Bearer test"})
    assert r.status_code == 400
    assert r.json()["error"]["code"] == "content_policy_violation"
    body = {"model": "gemini_decisions:blocked", "state": "x", "questions": {"is_spam": {"type": "noul", "instructions": "Spam?"}}}
    r = httpx.post(f"{base_url}/v1/systemone", json=body, headers={"authorization": "Bearer test"})
    assert r.status_code == 400
    assert "detail" in r.json()
