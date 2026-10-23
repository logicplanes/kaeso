from __future__ import annotations

import json

import openai
import pytest

from conftest import expected

SCENARIO_KINDS = ("generate",)
MESSAGES = [{"role": "user", "content": "What is the weather in Paris?"}]


def is_structured(name: str) -> bool:
    return any(k in name for k in ("json", "structured-output"))


def client(url: str) -> openai.OpenAI:
    return openai.OpenAI(base_url=url + "/v1", api_key="test", max_retries=0)


def chat_view(completion) -> dict:
    message = completion.choices[0].message
    tools = [
        {"name": t.function.name, "arguments": json.loads(t.function.arguments or "null")}
        for t in (message.tool_calls or [])
        if t.type == "function"
    ]
    return {"text": message.content or "", "tool_calls": tools}


def reasoning_text(item) -> str:
    parts = [s.text for s in item.summary] + [c.text for c in (item.content or [])]
    return "\n\n".join(p for p in parts if p)


def responses_view(response) -> dict:
    tools = [
        {"name": item.name, "arguments": json.loads(item.arguments or "null")}
        for item in response.output
        if item.type == "function_call"
    ]
    reasoning = [reasoning_text(i) for i in response.output if i.type == "reasoning"]
    return {
        "text": response.output_text,
        "tool_calls": tools,
        "reasoning": [r for r in reasoning if r],
    }


def test_chat_completions(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    if "error" in want:
        with pytest.raises(openai.APIStatusError) as info:
            c.chat.completions.create(model=model, messages=MESSAGES)
        assert info.value.body["message"] == want["error"]
        return
    if want.get("stream_error"):
        with pytest.raises(openai.APIError):
            with c.chat.completions.stream(model=model, messages=MESSAGES) as s:
                s.get_final_completion()
        return
    if scenario["stream"]:
        try:
            with c.chat.completions.stream(
                model=model, messages=MESSAGES, stream_options={"include_usage": True}
            ) as s:
                completion = s.get_final_completion()
        except (openai.LengthFinishReasonError, openai.ContentFilterFinishReasonError) as e:
            completion = e.completion
    else:
        completion = c.chat.completions.create(model=model, messages=MESSAGES)
    assert completion.object == "chat.completion"
    got = chat_view(completion)
    if not want.get("refusal"):
        assert got["text"] == want["text"]
    assert got["tool_calls"] == want["tool_calls"]
    if is_structured(scenario["name"]):
        json.loads(got["text"])


def test_responses(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    if "error" in want:
        with pytest.raises(openai.APIStatusError) as info:
            c.responses.create(model=model, input="What is the weather in Paris?")
        assert info.value.body["message"] == want["error"]
        return
    if want.get("stream_error"):
        with pytest.raises((openai.APIError, RuntimeError)):
            with c.responses.stream(model=model, input="What is the weather in Paris?") as s:
                s.get_final_response()
        return
    if scenario["stream"]:
        response = None
        streamed: dict[tuple[str, int], str] = {}
        done: dict[tuple[str, int], str] = {}
        items = []
        with c.responses.stream(model=model, input="What is the weather in Paris?") as s:
            for event in s:
                if event.type == "response.output_item.done":
                    items.append(event.item)
                elif event.type == "response.reasoning_summary_text.delta":
                    key = (event.item_id, event.summary_index)
                    streamed[key] = streamed.get(key, "") + event.delta
                elif event.type == "response.reasoning_summary_text.done":
                    done[(event.item_id, event.summary_index)] = event.text
                elif event.type in ("response.completed", "response.incomplete"):
                    response = event.response
        assert response is not None, "no final response event"
        if not response.output:
            response = response.model_copy(update={"output": items})
        assert streamed == done
        final = {
            (item.id, i): part.text
            for item in response.output
            if item.type == "reasoning"
            for i, part in enumerate(item.summary)
        }
        assert {k: v for k, v in final.items() if v} == {k: v for k, v in done.items() if v}
    else:
        response = c.responses.create(model=model, input="What is the weather in Paris?")
    assert response.object == "response"
    got = responses_view(response)
    if not want.get("refusal"):
        assert got["text"] == want["text"]
    assert got["tool_calls"] == want["tool_calls"]
    assert got["reasoning"] == want["reasoning"]
    if is_structured(scenario["name"]):
        json.loads(got["text"])


@pytest.mark.parametrize("source,name", [
    ("openai_responses", "input_tokens/string-input"),
    ("anthropic_messages", "count_tokens/basic-system-string"),
])
def test_input_tokens(base_url, source, name):
    want = expected(base_url, {"source": source, "name": name, "kind": "count_tokens", "stream": False})
    got = client(base_url).responses.input_tokens.count(
        model=f"{source}:{name}", input="What is the weather in Paris?"
    )
    assert got.input_tokens == want["input_tokens"]
