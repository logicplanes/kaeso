from __future__ import annotations

import json

import openai
import pytest

from conftest import FIXTURES, expected

SCENARIO_KINDS = ("decide", "error")
SCENARIO_SOURCES = ("openai_decisions",)
SOURCE = FIXTURES / "openai_decisions"
SENT = ("input", "questions", "prompt_cache_key", "prompt_cache_options", "safety_identifier")


def client(base_url: str) -> openai.OpenAI:
    return openai.OpenAI(base_url=base_url + "/v1", api_key="test", max_retries=0)


def close(a: float, b: float) -> bool:
    return a == pytest.approx(b, abs=1e-9)


def choice_confidence(ps: list[float]) -> float:
    n = len(ps)
    return 1.0 if n == 1 else max(0.0, (max(ps) - 1 / n) / (1 - 1 / n))


def score_confidence(ps: list[float]) -> float:
    n = len(ps)
    if n == 1:
        return 1.0
    m = max(range(n), key=lambda i: (ps[i], -i))
    spread = sum(p * abs(i - m) for i, p in enumerate(ps))
    middle = (n - 1) / 2
    return max(0.0, 1 - spread / (sum(abs(i - middle) for i in range(n)) / n))


def check_answer(question: dict, got, want: dict, recomputed: bool) -> None:
    assert got.name == question.get("name")
    if want["type"] == "refusal":
        assert got.type == "refusal"
    elif want["type"] == "predicate":
        assert got.type == "predicate"
        assert close(got.probability, want["probability"])
    elif want["type"] == "choice":
        values = [c["value"] for c in question["choices"]]
        assert got.type == "choice"
        assert got.choice == values[want["option"]]
        assert type(got.choice) is type(values[want["option"]])
        given = {json.dumps(p.value): p.probability for p in got.probabilities}
        wanted = {json.dumps(v): p for v, p in zip(values, want["probabilities"]) if p is not None}
        assert given.keys() == wanted.keys()
        for key, p in wanted.items():
            assert close(given[key], p)
        confidence = choice_confidence([p or 0.0 for p in want["probabilities"]]) if recomputed else want["confidence"]
        assert close(got.confidence, confidence)
    elif want["type"] == "score":
        labels = [level["label"] for level in question["levels"]]
        assert got.type == "score"
        assert close(got.score, want["score"])
        for p in got.probabilities:
            assert p.label == labels[p.value]
            assert close(p.probability, want["probabilities"][p.value])
        confidence = score_confidence([p or 0.0 for p in want["probabilities"]]) if recomputed else want["confidence"]
        assert close(got.confidence, confidence)
    else:
        raise AssertionError(want)


def test_openai_decisions(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    if scenario["kind"] == "error":
        with pytest.raises(openai.APIStatusError) as info:
            c.decisions.create(model=model, input="hi", questions=[{"type": "predicate", "instructions": "Hi?"}])
        assert info.value.status_code == want["status"], want
        assert isinstance(info.value.body, dict) and "message" in info.value.body
        if want["status"] == 429 and want.get("retry_after"):
            assert info.value.response.headers.get("retry-after")
        return
    req = json.loads((SOURCE / "requests" / f"{scenario['name']}.json").read_text())
    known = {k: v for k, v in req.items() if k in SENT}
    extra = {k: v for k, v in req.items() if k not in SENT and k != "model"}
    got = c.decisions.create(model=model, extra_body=extra or None, **known)
    assert got.model == want["model"]
    assert got.usage.input_tokens == want["input_tokens"]
    assert got.usage.output_tokens == want["output_tokens"]
    assert len(got.answers) == len(req["questions"])
    for question, answer, w in zip(req["questions"], got.answers, want["answers"]):
        check_answer(question, answer, w, recomputed=False)


def system_one_as_openai(name: str) -> dict:
    body = json.loads((FIXTURES / "system_one" / "requests" / f"{name}.json").read_text())
    questions = []
    for key, q in body["questions"].items():
        out = {"type": {"noul": "predicate"}.get(q["type"], q["type"]), "name": key, "instructions": q["instructions"]}
        if q["type"] == "choice":
            out["choices"] = [{"value": v, "description": d} for v, d in q["criteria"].items()]
        if q["type"] == "score":
            out["levels"] = [{"label": level} for level in q["criteria"]]
        questions.append(out)
    return {"input": body["state"], "questions": questions}


@pytest.mark.parametrize("name", ["noul", "choice", "score"])
def test_system_one_answers_reach_openai_apps(base_url, name):
    scenario = {"source": "system_one", "name": name, "kind": "decide", "stream": False}
    want = expected(base_url, scenario)
    req = system_one_as_openai(name)
    got = client(base_url).decisions.create(model=f"system_one:{name}", **req)
    assert got.model == want["model"]
    assert got.usage.input_tokens == want["input_tokens"]
    assert got.usage.total_tokens == want["input_tokens"] + want["output_tokens"]
    for question, answer, w in zip(req["questions"], got.answers, want["answers"]):
        check_answer(question, answer, w, recomputed=True)
