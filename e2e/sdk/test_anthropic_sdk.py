import base64

import anthropic
import pytest

TINY_PNG = base64.b64encode(
    bytes.fromhex(
        "89504e470d0a1a0a0000000d4948445200000001000000010806000000"
        "1f15c4890000000d49444154789c6360000002000154a24f5f0000000049454e44ae426082"
    )
).decode()
TINY_PDF = base64.b64encode(b"%PDF-1.4\n1 0 obj<<>>endobj\ntrailer<<>>\n%%EOF").decode()


def user(text: str) -> list:
    return [{"role": "user", "content": text}]


def test_messages(an, mock):
    r = an.messages.create(model="claude-haiku-4-5", max_tokens=100, messages=user("Hello"))
    assert r.type == "message" and r.content[0].type == "text"
    sent = mock.last()
    assert sent["path"] == "/v1/messages"
    assert sent["headers"]["x-api-key"].startswith("sk-ant-test")


def test_alias_and_prefixed_names(an, mock):
    an.messages.create(model="chat-default", max_tokens=50, messages=user("Hi"))
    assert mock.last()["body"]["model"] == "claude-haiku-4-5"
    an.messages.create(model="anthropic/claude-haiku-4-5", max_tokens=50, messages=user("Hi"))
    assert mock.last()["body"]["model"] == "claude-haiku-4-5"


def test_streaming(an, mock):
    with an.messages.stream(model="claude-haiku-4-5", max_tokens=100, messages=user("Hello")) as s:
        text = "".join(s.text_stream)
        final = s.get_final_message()
    assert text and final.stop_reason == "end_turn"
    assert final.usage.output_tokens > 0


def test_streaming_from_an_openai_model(an, mock):
    with an.messages.stream(model="gpt-4o", max_tokens=100, messages=user("Hello")) as s:
        text = "".join(s.text_stream)
    assert text
    assert mock.last()["path"] == "/v1/responses"


def test_streams_through_a_quiet_provider(an, mock):
    for model in ("claude-haiku-4-5", "gpt-4o"):
        with an.messages.stream(model=model, max_tokens=100, messages=user("Hello [mock:silence=2500]")) as s:
            text = "".join(s.text_stream)
            final = s.get_final_message()
        assert text and final.stop_reason == "end_turn"


def test_tools(an, mock):
    tools = [{
        "name": "get_weather",
        "description": "Weather for a place",
        "input_schema": {"type": "object", "properties": {"location": {"type": "string"}}, "required": ["location"]},
    }]
    r = an.messages.create(model="claude-haiku-4-5", max_tokens=200, tools=tools, messages=user("Weather? [mock:fixture=tools-single]"))
    assert r.stop_reason == "tool_use"
    assert any(b.type == "tool_use" for b in r.content)
    r = an.messages.create(model="gpt-4o", max_tokens=200, tools=tools, messages=user("Weather? [mock:fixture=tools-function]"))
    assert any(b.type == "tool_use" for b in r.content)
    assert mock.last()["body"]["tools"][0]["type"] == "function"


def test_tool_search_on_an_openai_model(an, mock):
    schema = {"type": "object", "properties": {"query": {"type": "string"}}, "required": ["query"]}
    tools = [
        {"name": "ToolSearch", "description": "Load deferred tools", "input_schema": schema},
        {"name": "get_weather", "description": "Weather for a place", "input_schema": schema, "defer_loading": True},
    ]
    an.beta.messages.create(
        model="gpt-4o",
        max_tokens=200,
        betas=["advanced-tool-use-2025-11-20"],
        tools=tools,
        messages=user("Weather? [mock:fixture=tools-function]"),
    )
    assert [t["name"] for t in mock.last()["body"]["tools"]] == ["ToolSearch"]
    r = an.beta.messages.create(
        model="gpt-4o",
        max_tokens=200,
        betas=["advanced-tool-use-2025-11-20"],
        tools=tools,
        messages=[
            {"role": "user", "content": "Weather?"},
            {"role": "assistant", "content": [{"type": "tool_use", "id": "toolu_1", "name": "ToolSearch", "input": {"query": "weather"}}]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": [{"type": "tool_reference", "tool_name": "get_weather"}]},
                {"type": "text", "text": "Go on. [mock:fixture=tools-function]"},
            ]},
        ],
    )
    assert any(b.type == "tool_use" for b in r.content)
    sent = mock.last()["body"]
    assert [t["name"] for t in sent["tools"]] == ["ToolSearch", "get_weather"]
    assert "defer_loading" not in sent["tools"][1]
    output = next(i for i in sent["input"] if i.get("type") == "function_call_output")
    assert output["output"] == "get_weather"


def test_images_and_pdfs(an, mock):
    content = [
        {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": TINY_PNG}},
        {"type": "text", "text": "Describe. [mock:fixture=image-base64]"},
    ]
    r = an.messages.create(model="claude-haiku-4-5", max_tokens=100, messages=[{"role": "user", "content": content}])
    assert r.content
    content = [
        {"type": "document", "source": {"type": "base64", "media_type": "application/pdf", "data": TINY_PDF}},
        {"type": "text", "text": "Summarise. [mock:fixture=document-pdf-base64]"},
    ]
    r = an.messages.create(model="claude-haiku-4-5", max_tokens=100, messages=[{"role": "user", "content": content}])
    assert r.content


def test_thinking(an, mock):
    r = an.messages.create(
        model="claude-haiku-4-5",
        max_tokens=4000,
        thinking={"type": "enabled", "budget_tokens": 2048},
        messages=user("Think. [mock:fixture=thinking-enabled-budget]"),
    )
    assert any(b.type == "thinking" for b in r.content)
    assert mock.last()["body"]["thinking"]["budget_tokens"] == 2048
    an.messages.create(
        model="claude-sonnet-4-6",
        max_tokens=4000,
        thinking={"type": "adaptive"},
        output_config={"effort": "low"},
        messages=user("Think. [mock:fixture=thinking-adaptive-effort]"),
    )
    assert mock.last()["body"]["thinking"]["type"] == "adaptive"


def test_count_tokens(an, mock):
    r = an.messages.count_tokens(model="claude-haiku-4-5", messages=user("Hi"))
    assert r.input_tokens > 0
    r = an.messages.count_tokens(model="gpt-4o", messages=user("Hi"))
    assert r.input_tokens > 0
    assert mock.last()["path"] == "/v1/responses/input_tokens"


def test_models(an, install):
    ids = [m.id for m in an.models.list()]
    assert "claude-haiku-4-5" in ids and "gpt-4o" in ids and "chat-default" in ids
    m = an.models.retrieve("claude-haiku-4-5")
    assert m.display_name == "Claude Haiku 4.5 (latest)"
    with pytest.raises(anthropic.NotFoundError):
        an.models.retrieve("claude-opus-9")


def test_readable_thinking_from_openai_for_an_anthropic_app(an, mock):
    with an.messages.stream(
        model="gpt-5-mini",
        max_tokens=2000,
        thinking={"type": "enabled", "budget_tokens": 1024},
        messages=[{"role": "user", "content": "17 * 24? [mock:fixture=reasoning-summary-stream]"}],
    ) as s:
        message = s.get_final_message()
    sent = mock.last()["body"]
    assert sent["reasoning"] == {"effort": "low", "summary": "auto"}
    thinking = [b for b in message.content if b.type == "thinking"]
    assert thinking and "Multiplying 17 by 24" in thinking[0].thinking
    assert thinking[0].signature.startswith("kpp:reasoning:v1:")
