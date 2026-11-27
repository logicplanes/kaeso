import base64
from typing import Optional

import pydantic
import pytest
from google import genai
from google.genai import errors, types

WEATHER = types.Tool(
    function_declarations=[
        types.FunctionDeclaration(
            name="get_weather",
            description="Weather for a place",
            parameters_json_schema={"type": "object", "properties": {"location": {"type": "string"}}, "required": ["location"]},
        )
    ]
)
TIME = types.Tool(
    function_declarations=[
        types.FunctionDeclaration(
            name="get_time",
            description="Local time",
            parameters_json_schema={"type": "object", "properties": {"timezone": {"type": "string"}}},
        )
    ]
)


@pytest.mark.parametrize(
    ("model", "path"),
    [
        ("claude-haiku-4-5", "/v1/messages"),
        ("gpt-4o", "/v1/responses"),
        ("gemini-3.5-flash", "/v1beta/models/gemini-3.5-flash:generateContent"),
    ],
)
def test_generate_content_reaches_every_provider(gg: genai.Client, mock, model, path):
    r = gg.models.generate_content(model=model, contents="Hello")
    assert r.text
    assert r.candidates[0].finish_reason == types.FinishReason.STOP
    assert r.usage_metadata.prompt_token_count > 0
    sent = mock.last()
    assert sent["path"] == path
    assert all("sk-kso-" not in v for v in sent["headers"].values())


def test_the_key_reaches_google_only_in_its_header(gg: genai.Client, mock):
    gg.models.generate_content(model="gemini-3.5-flash", contents="Hello [mock:fixture=recorded-text-simple]")
    sent = mock.last()
    assert sent["headers"]["x-goog-api-key"].startswith("AIzaSyTest")
    assert "authorization" not in sent["headers"]
    assert not sent.get("query")


@pytest.mark.parametrize("model", ["claude-haiku-4-5", "gpt-4o", "gemini-3.5-flash"])
def test_generate_content_stream(gg: genai.Client, mock, model):
    chunks = list(gg.models.generate_content_stream(model=model, contents="Hello"))
    assert "".join(c.text or "" for c in chunks)
    assert chunks[-1].candidates[0].finish_reason == types.FinishReason.STOP
    assert chunks[-1].usage_metadata.candidates_token_count > 0


def test_streams_through_a_quiet_provider(gg: genai.Client, mock):
    chunks = list(gg.models.generate_content_stream(model="claude-haiku-4-5", contents="Hello [mock:silence=2500]"))
    assert "".join(c.text or "" for c in chunks)


def test_a_tool_loop_on_gemini_sends_its_thought_signature_back(gg: genai.Client, mock):
    config = types.GenerateContentConfig(tools=[WEATHER])
    first = gg.models.generate_content(
        model="gemini-3.5-flash", contents="Weather in Paris? [mock:fixture=recorded-tools-single]", config=config
    )
    call = first.function_calls[0]
    assert call.name == "get_weather" and call.args == {"location": "Paris"}
    signed = first.candidates[0].content
    assert signed.parts[0].thought_signature
    gg.models.generate_content(
        model="gemini-3.5-flash",
        contents=[
            types.Content(role="user", parts=[types.Part(text="Weather in Paris?")]),
            signed,
            types.Content(
                role="user",
                parts=[types.Part(function_response=types.FunctionResponse(id=call.id, name=call.name, response={"output": "18 °C"}))],
            ),
        ],
        config=config,
    )
    sent = mock.last()["body"]["contents"]
    replayed = sent[1]["parts"][0]
    assert replayed["functionCall"]["name"] == "get_weather"
    assert base64.urlsafe_b64decode(replayed["thoughtSignature"]) == signed.parts[0].thought_signature
    assert sent[2]["parts"][0]["functionResponse"]["response"] == {"output": "18 °C"}


def test_a_tool_loop_on_claude_keeps_claudes_thinking(gg: genai.Client, mock):
    config = types.GenerateContentConfig(tools=[TIME], thinking_config=types.ThinkingConfig(thinking_level=types.ThinkingLevel.LOW))
    first = gg.models.generate_content(
        model="claude-sonnet-4-6",
        contents="Is it daytime in Paris? [mock:fixture=thinking-interleaved-tools]",
        config=config,
    )
    call = first.function_calls[0]
    assert call.id == "toolu_01afnDK4mGFJyw1zWvMT248V"
    gg.models.generate_content(
        model="claude-sonnet-4-6",
        contents=[
            types.Content(role="user", parts=[types.Part(text="Is it daytime in Paris?")]),
            first.candidates[0].content,
            types.Content(
                role="user",
                parts=[types.Part(function_response=types.FunctionResponse(id=call.id, name=call.name, response={"output": "14:05"}))],
            ),
        ],
        config=config,
    )
    sent = mock.last()
    assert sent["path"] == "/v1/messages"
    turn = next(m for m in sent["body"]["messages"] if m["role"] == "assistant")
    assert turn["content"][0]["type"] == "thinking"
    assert turn["content"][0]["signature"].startswith("EqEvMLy0")
    assert turn["content"][-1]["id"] == call.id


class City(pydantic.BaseModel):
    name: str
    population: Optional[int] = None


def test_geminis_own_schemas_reach_other_formats_as_json_schema(gg: genai.Client, mock):
    config = types.GenerateContentConfig(
        response_mime_type="application/json",
        response_schema=City,
        max_output_tokens=200,
    )
    gg.models.generate_content(model="claude-haiku-4-5", contents="Tokyo [mock:fixture=structured-output-json-schema]", config=config)
    schema = mock.last()["body"]["output_config"]["format"]["schema"]
    assert schema["type"] == "object"
    assert list(schema["properties"]) == ["name", "population"]
    assert schema["properties"]["population"]["type"] == ["integer", "null"]
    assert "property_ordering" not in schema and "propertyOrdering" not in schema
    weather = types.FunctionDeclaration(
        name="get_weather",
        description="Weather for a place",
        parameters=types.Schema(type="OBJECT", properties={"location": types.Schema(type="STRING")}, required=["location"]),
    )
    config = types.GenerateContentConfig(tools=[types.Tool(function_declarations=[weather])])
    gg.models.generate_content(model="gpt-4o", contents="Weather? [mock:fixture=tools-function]", config=config)
    assert mock.last()["body"]["tools"][0]["parameters"] == {
        "properties": {"location": {"type": "string"}},
        "required": ["location"],
        "type": "object",
    }


def get_weather(location: str, unit: str = "celsius") -> str:
    """Weather for a place."""
    return f"Sunny, 21 degrees {unit}"


def test_automatic_function_calling_works_on_other_formats(gg: genai.Client, mock):
    config = types.GenerateContentConfig(
        tools=[get_weather], automatic_function_calling=types.AutomaticFunctionCallingConfig(maximum_remote_calls=2)
    )
    r = gg.models.generate_content(model="claude-haiku-4-5", contents="Weather? [mock:fixture=tools-single]", config=config)
    first, second = mock.received()[-2:]
    assert first["body"]["tools"][0]["input_schema"]["properties"]["location"] == {"type": "string"}
    assert "response_json_schema" not in first["body"]["tools"][0]
    result = second["body"]["messages"][-1]["content"][0]
    assert result["type"] == "tool_result" and result["tool_use_id"] == "toolu_01VLLqZ1bfs3WXvVeYqZPtE7"
    assert "Sunny, 21 degrees fahrenheit" in str(result["content"])
    assert r.sdk_http_response.headers["kaeso-left-out"] == "tools[0].functionDeclarations[0].responseJsonSchema"
    assert len(r.automatic_function_calling_history) == 3


def test_tools_reach_other_formats(gg: genai.Client, mock):
    config = types.GenerateContentConfig(tools=[WEATHER])
    r = gg.models.generate_content(model="gpt-4o", contents="Weather? [mock:fixture=tools-function]", config=config)
    assert r.function_calls
    assert mock.last()["body"]["tools"][0]["type"] == "function"
    r = gg.models.generate_content(model="claude-haiku-4-5", contents="Weather? [mock:fixture=tools-single]", config=config)
    assert r.function_calls and r.candidates[0].finish_reason == types.FinishReason.STOP


def test_top_k_is_left_out_for_a_model_without_it(gg: genai.Client, mock):
    config = types.GenerateContentConfig(temperature=1, top_p=0.95, top_k=64)
    r = gg.models.generate_content(model="gpt-4o", contents="Hello", config=config)
    assert r.text
    assert r.sdk_http_response.headers["kaeso-left-out"] == "generationConfig.topK"
    sent = mock.last()["body"]
    assert "top_k" not in sent and sent["top_p"] == 0.95
    gg.models.generate_content(model="gemini-3.5-flash", contents="Hello [mock:fixture=recorded-text-simple]", config=config)
    assert mock.last()["body"]["generationConfig"]["topK"] == 64


@pytest.mark.parametrize(("model", "path"), [("claude-haiku-4-5", "/v1/messages/count_tokens"), ("gemini-3.5-flash", "/v1beta/models/gemini-3.5-flash:countTokens")])
def test_count_tokens(gg: genai.Client, mock, model, path):
    r = gg.models.count_tokens(model=model, contents="Hello")
    assert r.total_tokens > 0
    assert mock.last()["path"] == path


def test_embed_content(gg: genai.Client, mock):
    r = gg.models.embed_content(model="text-embedding-3-small", contents=["refund policy", "shipping times [mock:fixture=recorded-string-array]"])
    assert len(r.embeddings) == 2 and r.embeddings[0].values
    assert mock.last()["path"] == "/v1/embeddings"
    assert mock.last()["body"]["input"][0] == "refund policy"
    r = gg.models.embed_content(model="gemini-embedding-2", contents="refund policy [mock:fixture=recorded-embed-single]")
    assert r.embeddings[0].values
    sent = mock.last()
    assert sent["path"] == "/v1beta/models/gemini-embedding-2:batchEmbedContents"
    assert sent["body"]["requests"][0]["content"]["role"] == "user"


def test_embeddings_and_generation_models_stay_apart(gg: genai.Client):
    with pytest.raises(errors.ClientError) as e:
        gg.models.embed_content(model="claude-haiku-4-5", contents="hello")
    assert e.value.code == 400 and "cannot create embeddings" in e.value.message


def test_models(gg: genai.Client):
    names = [m.name for m in gg.models.list()]
    assert {"models/gpt-4o", "models/claude-haiku-4-5", "models/gemini-3.5-flash", "models/chat-default"} <= set(names)
    paged = gg.models.list(config={"page_size": 1})
    assert len(paged.page) == 1
    assert [m.name for m in paged] == names
    m = gg.models.get(model="claude-haiku-4-5")
    assert m.name == "models/claude-haiku-4-5" and m.input_token_limit > 0
    with pytest.raises(errors.ClientError) as e:
        gg.models.get(model="gpt-9")
    assert e.value.code == 404 and e.value.status == "NOT_FOUND"


def test_errors_are_in_geminis_shape(install, gg: genai.Client, gemini_version):
    stranger = genai.Client(
        api_key="sk-kso-not-a-key",
        http_options=types.HttpOptions(
            base_url=install.url, api_version=gemini_version, retry_options=types.HttpRetryOptions(attempts=1)
        ),
    )
    with pytest.raises(errors.ClientError) as e:
        stranger.models.generate_content(model="claude-haiku-4-5", contents="Hello")
    assert e.value.code == 401 and e.value.status == "UNAUTHENTICATED"
    with pytest.raises(errors.ClientError) as e:
        gg.models.generate_content(model="gpt-9", contents="Hello")
    assert e.value.code == 404 and e.value.status == "NOT_FOUND"
    with pytest.raises(errors.ClientError) as e:
        gg.models.generate_content(
            model="claude-haiku-4-5", contents="Hello", config=types.GenerateContentConfig(candidate_count=2)
        )
    assert e.value.code == 400 and e.value.status == "INVALID_ARGUMENT"
    with pytest.raises(errors.ServerError) as e:
        gg.models.generate_content(model="claude-haiku-4-5", contents="Hello [mock:error=overloaded]")
    assert e.value.code == 503 and e.value.status == "UNAVAILABLE"


def test_the_sdks_default_version_is_v1(install, mock):
    seen = []
    default = genai.Client(
        api_key=install.key,
        http_options=types.HttpOptions(
            base_url=install.url, client_args={"event_hooks": {"request": [lambda r: seen.append(r.url.path)]}}
        ),
    )
    assert default.models.generate_content(model="claude-haiku-4-5", contents="Hello").text
    assert seen == ["/v1/models/claude-haiku-4-5:generateContent"]
