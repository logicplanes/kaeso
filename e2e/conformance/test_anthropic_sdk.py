from __future__ import annotations

import json

import anthropic
import pytest

from conftest import expected

SCENARIO_KINDS = ("generate",)
MESSAGES = [{"role": "user", "content": "What is the weather in Paris?"}]


def is_structured(name: str) -> bool:
    return any(k in name for k in ("json", "structured-output"))


def client(url: str) -> anthropic.Anthropic:
    return anthropic.Anthropic(base_url=url, api_key="test", max_retries=0)


def view(message) -> dict:
    text = "".join(b.text for b in message.content if b.type == "text")
    tools = [
        {"name": b.name, "arguments": b.input}
        for b in message.content
        if b.type == "tool_use"
    ]
    reasoning = [b.thinking for b in message.content if b.type == "thinking" and b.thinking]
    return {"text": text, "tool_calls": tools, "reasoning": reasoning}


def test_messages(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    if "error" in want:
        with pytest.raises(anthropic.APIStatusError) as info:
            c.messages.create(model=model, max_tokens=64, messages=MESSAGES)
        assert info.value.body["error"]["message"] == want["error"]
        return
    if want.get("stream_error"):
        with pytest.raises(anthropic.APIError):
            with c.messages.stream(model=model, max_tokens=64, messages=MESSAGES) as s:
                s.get_final_message()
        return
    if scenario["stream"]:
        with c.messages.stream(model=model, max_tokens=64, messages=MESSAGES) as s:
            message = s.get_final_message()
    else:
        message = c.messages.create(model=model, max_tokens=64, messages=MESSAGES)
    assert message.type == "message"
    assert message.role == "assistant"
    got = view(message)
    if not want.get("refusal"):
        assert got["text"] == want["text"]
    assert got["tool_calls"] == want["tool_calls"]
    assert got["reasoning"] == want["reasoning"]
    if is_structured(scenario["name"]):
        json.loads(got["text"])


@pytest.mark.parametrize("source,name", [
    ("anthropic_messages", "count_tokens/basic-system-string"),
    ("openai_responses", "input_tokens/string-input"),
])
def test_count_tokens(base_url, source, name):
    want = expected(base_url, {"source": source, "name": name, "kind": "count_tokens", "stream": False})
    got = client(base_url).messages.count_tokens(model=f"{source}:{name}", messages=MESSAGES)
    assert got.input_tokens == want["input_tokens"]
