from __future__ import annotations

import json

import pytest
from google import genai
from google.genai import errors, types

from conftest import expected

SCENARIO_KINDS = ("generate",)
PROMPT = "What is the weather in Paris?"


def is_structured(name: str) -> bool:
    return any(k in name for k in ("json", "structured-output"))


def client(url: str) -> genai.Client:
    return genai.Client(
        api_key="test",
        http_options=types.HttpOptions(base_url=url, api_version="v1beta"),
    )


def collect(parts, view: dict) -> None:
    for part in parts or []:
        if part.function_call is not None:
            view["tool_calls"].append(
                {"name": part.function_call.name, "arguments": part.function_call.args or {}}
            )
        elif part.text is not None and part.thought:
            if part.text:
                view["reasoning"].append(part.text)
        elif part.text is not None:
            view["text"] += part.text


def empty_view() -> dict:
    return {"text": "", "tool_calls": [], "reasoning": []}


def parts_of(response):
    candidates = response.candidates or []
    if not candidates or candidates[0].content is None:
        return []
    return candidates[0].content.parts


def test_generate_content(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    if "error" in want:
        with pytest.raises(errors.APIError) as info:
            c.models.generate_content(model=model, contents=PROMPT)
        assert info.value.message == want["error"]
        return
    if want.get("stream_error"):
        with pytest.raises(errors.APIError):
            for _ in c.models.generate_content_stream(model=model, contents=PROMPT):
                pass
        return
    got = empty_view()
    if scenario["stream"]:
        for chunk in c.models.generate_content_stream(model=model, contents=PROMPT):
            collect(parts_of(chunk), got)
    else:
        collect(parts_of(c.models.generate_content(model=model, contents=PROMPT)), got)
    if not want.get("refusal"):
        assert got["text"] == want["text"]
    assert got["tool_calls"] == want["tool_calls"]
    if scenario["source"] == "gemini":
        thoughts = [r for r in want["reasoning"] if r]
        assert "".join(got["reasoning"]) == "".join(thoughts)
    if is_structured(scenario["name"]) and got["text"]:
        json.loads(got["text"])


@pytest.mark.parametrize("source,name", [
    ("gemini", "count_tokens/recorded-contents"),
    ("anthropic_messages", "count_tokens/basic-system-string"),
    ("openai_responses", "input_tokens/string-input"),
])
def test_count_tokens(base_url, source, name):
    want = expected(base_url, {"source": source, "name": name, "kind": "count_tokens", "stream": False})
    c = client(base_url)
    got = c.models.count_tokens(model=f"{source}:{name}", contents=PROMPT)
    assert got.total_tokens == want["input_tokens"]


def test_models_page_with_tokens(base_url):
    c = client(base_url)
    names = [m.name for m in c.models.list(config={"page_size": 2})]
    assert names == [
        "models/claude-opus-5",
        "models/claude-sonnet-5",
        "models/gpt-6-astra",
        "models/claude-sonnet-4-5-20250929",
    ]
    model = c.models.get(model="claude-opus-5")
    assert (model.display_name, model.input_token_limit, model.output_token_limit) == (
        "Claude Opus 5",
        1_000_000,
        128_000,
    )
    with pytest.raises(errors.APIError) as info:
        c.models.get(model="no-such-model")
    assert info.value.code == 404


def test_audio_and_video_parts_reach_kpp(base_url):
    want = expected(base_url, {"source": "gemini", "name": "recorded-video-input", "kind": "generate", "stream": False})
    contents = [
        types.Part.from_bytes(data=b"ID3\x04\x00", mime_type="audio/mpeg"),
        types.Part(
            file_data=types.FileData(file_uri="https://www.youtube.com/watch?v=UTdfxFyOQTI"),
            video_metadata=types.VideoMetadata(fps=0.5),
        ),
        "What color fills this video? Answer in one word.",
    ]
    c = client(base_url)
    response = c.models.generate_content(model="gemini:recorded-video-input", contents=contents)
    got = empty_view()
    collect(parts_of(response), got)
    assert got["text"] == want["text"]


def test_speech_reaches_a_gemini_app_from_a_chat_audio_reply(base_url):
    c = client(base_url)
    config = types.GenerateContentConfig(
        response_modalities=["AUDIO"],
        speech_config=types.SpeechConfig(
            voice_config=types.VoiceConfig(prebuilt_voice_config=types.PrebuiltVoiceConfig(voice_name="alloy"))
        ),
    )
    response = c.models.generate_content(model="openai_chat:audio-output", contents="Say hello.", config=config)
    parts = parts_of(response)
    assert parts[0].inline_data.data.startswith(b"RIFF")
