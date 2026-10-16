import base64

import openai
import pytest

from test_anthropic_sdk import TINY_PDF, TINY_PNG


def user(text: str) -> list:
    return [{"role": "user", "content": text}]


def test_chat_completions(oa, mock):
    r = oa.chat.completions.create(model="gpt-4o", messages=user("Hello"))
    assert r.object == "chat.completion" and r.choices[0].message.content
    sent = mock.last()
    assert sent["path"] == "/v1/responses"
    assert sent["headers"]["authorization"] == "Bearer sk-test-openai-e2e-credential"


def test_chat_to_an_anthropic_model(oa, mock):
    r = oa.chat.completions.create(model="claude-haiku-4-5", messages=user("Hello"))
    assert r.choices[0].message.content
    sent = mock.last()
    assert sent["path"] == "/v1/messages"
    assert sent["body"]["max_tokens"] == 64000


def test_chat_streaming(oa, mock):
    chunks = list(oa.chat.completions.create(model="claude-haiku-4-5", messages=user("Hello"), stream=True))
    assert "".join(c.choices[0].delta.content or "" for c in chunks if c.choices)
    chunks = list(oa.chat.completions.create(
        model="gpt-4o", messages=user("Hello"), stream=True, stream_options={"include_usage": True}
    ))
    assert chunks[-1].usage is not None


def test_responses(oa, mock):
    r = oa.responses.create(model="gpt-4o", input="Hello")
    assert r.output_text
    r = oa.responses.create(model="claude-haiku-4-5", input="Hello")
    assert r.output_text
    assert mock.last()["path"] == "/v1/messages"
    with oa.responses.stream(model="gpt-4o", input="Hello") as s:
        events = [e.type for e in s]
    assert "response.completed" in events


def test_streams_through_a_quiet_provider(oa, mock):
    chunks = list(oa.chat.completions.create(
        model="claude-haiku-4-5", messages=user("Hello [mock:silence=2500]"), stream=True
    ))
    assert "".join(c.choices[0].delta.content or "" for c in chunks if c.choices)
    with oa.responses.stream(model="claude-haiku-4-5", input="Hello [mock:silence=2500]") as s:
        events = [e.type for e in s]
    assert "response.completed" in events


def test_tools(oa, mock):
    tools = [{
        "type": "function",
        "function": {
            "name": "get_weather",
            "parameters": {"type": "object", "properties": {"location": {"type": "string"}}, "required": ["location"]},
        },
    }]
    r = oa.chat.completions.create(model="claude-haiku-4-5", tools=tools, messages=user("Weather? [mock:fixture=tools-single]"))
    assert r.choices[0].finish_reason == "tool_calls"
    assert r.choices[0].message.tool_calls
    r = oa.responses.create(
        model="gpt-4o",
        input="Weather? [mock:fixture=tools-function]",
        tools=[{"type": "function", "name": "get_weather", "parameters": tools[0]["function"]["parameters"]}],
    )
    assert any(o.type == "function_call" for o in r.output)


def test_images_and_pdfs(oa, mock):
    content = [
        {"type": "text", "text": "Describe."},
        {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{TINY_PNG}"}},
    ]
    r = oa.chat.completions.create(model="claude-haiku-4-5", messages=[{"role": "user", "content": content}])
    assert r.choices[0].message.content
    assert mock.last()["body"]["messages"][0]["content"][1]["type"] == "image"
    content = [
        {"type": "text", "text": "Summarise."},
        {"type": "file", "file": {"filename": "a.pdf", "file_data": f"data:application/pdf;base64,{TINY_PDF}"}},
    ]
    r = oa.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": content}])
    assert r.choices[0].message.content


def test_reasoning(oa, mock):
    r = oa.responses.create(model="gpt-5-mini", input="Think. [mock:fixture=reasoning-encrypted]", reasoning={"effort": "high"})
    assert any(o.type == "reasoning" for o in r.output)
    assert mock.last()["body"]["reasoning"]["effort"] == "high"
    oa.chat.completions.create(model="claude-haiku-4-5", messages=user("Think."), reasoning_effort="low")
    thinking = mock.last()["body"]["thinking"]
    assert thinking["type"] == "enabled" and thinking["budget_tokens"] == 2048
    with pytest.raises(openai.BadRequestError) as e:
        oa.chat.completions.create(model="gpt-5-mini", messages=user("Think."), reasoning_effort="xhigh")
    assert "`minimal`, `low`, `medium`, `high`" in str(e.value)


def test_reasoning_summaries_from_claude_through_a_tool_loop(oa, mock):
    tools = [{"type": "function", "name": "get_weather", "description": "Weather in a city",
              "parameters": {"type": "object", "properties": {"city": {"type": "string"}}, "required": ["city"]}}]
    deltas = []
    with oa.responses.stream(
        model="claude-sonnet-4-6",
        input="Aurora tonight? [mock:fixture=thinking-redacted]",
        tools=tools,
        reasoning={"effort": "medium", "summary": "auto"},
        include=["reasoning.encrypted_content"],
        store=False,
    ) as s:
        for event in s:
            if event.type == "response.reasoning_summary_text.delta":
                deltas.append(event.delta)
        response = s.get_final_response()
    sent = mock.last()["body"]
    assert sent["thinking"] == {"type": "adaptive", "display": "summarized"}
    assert sent["output_config"] == {"effort": "medium"}
    reasoning = [o for o in response.output if o.type == "reasoning"]
    assert reasoning and reasoning[0].summary[0].text == "".join(deltas) != ""
    assert reasoning[0].encrypted_content.startswith("kpp:reasoning:v1:")
    call = next(o for o in response.output if o.type == "function_call")

    items = [{"role": "user", "content": "Aurora tonight?"}]
    items += [o.model_dump(exclude_none=True) for o in response.output]
    items.append({"type": "function_call_output", "call_id": call.call_id, "output": "clear skies"})
    oa.responses.create(model="claude-sonnet-4-6", input=items, tools=tools,
                        reasoning={"effort": "medium", "summary": "auto"}, store=False)
    turn = mock.last()["body"]["messages"][1]["content"]
    assert [b["type"] for b in turn] == ["thinking", "redacted_thinking", "tool_use"]
    assert turn[0]["signature"] and not turn[0]["signature"].startswith("kpp:")
    assert turn[2]["id"] == call.call_id


def test_token_counting(oa, mock):
    r = oa.responses.input_tokens.count(model="gpt-4o", input="Hello")
    assert r.input_tokens > 0
    r = oa.responses.input_tokens.count(model="claude-haiku-4-5", input="Hello")
    assert r.input_tokens > 0
    assert mock.last()["path"] == "/v1/messages/count_tokens"


def test_embeddings(oa, mock):
    r = oa.embeddings.create(model="text-embedding-3-small", input="hello")
    assert len(r.data[0].embedding) > 0
    with pytest.raises(openai.BadRequestError) as e:
        oa.embeddings.create(model="claude-haiku-4-5", input="hello")
    assert "cannot create embeddings" in str(e.value)


def test_models(oa):
    ids = [m.id for m in oa.models.list()]
    assert {"gpt-4o", "claude-haiku-4-5", "chat-default"} <= set(ids)
    m = oa.models.retrieve("claude-haiku-4-5")
    assert m.owned_by == "anthropic" and m.created > 0
    with pytest.raises(openai.NotFoundError):
        oa.models.retrieve("gpt-9")
