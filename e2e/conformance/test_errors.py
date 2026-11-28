from __future__ import annotations

import anthropic
import openai
import pytest
from google import genai
from google.genai import errors, types

from conftest import expected

SCENARIO_KINDS = ("error",)
SCENARIO_SOURCES = (
    "anthropic_messages",
    "openai_chat",
    "openai_responses",
    "openai_embeddings",
    "gemini",
)
MESSAGES = [{"role": "user", "content": "hi"}]

ANTHROPIC_STATUS = {
    "invalid_request": {400}, "context_too_long": {400}, "unsupported": {400},
    "content_filtered": {400}, "authentication": {401}, "billing": {402},
    "permission_denied": {403}, "not_found": {404}, "request_too_large": {413},
    "rate_limited": {429}, "provider_error": {500}, "timeout": {504}, "overloaded": {529},
}
GEMINI_STATUS = {
    "invalid_request": {400}, "context_too_long": {400}, "unsupported": {400},
    "content_filtered": {400}, "authentication": {400, 401}, "billing": {402},
    "permission_denied": {403}, "not_found": {404}, "request_too_large": {413},
    "rate_limited": {429}, "provider_error": {500}, "timeout": {504}, "overloaded": {503},
}
OPENAI_STATUS = {
    "invalid_request": {400, 409, 422}, "context_too_long": {400}, "unsupported": {400},
    "content_filtered": {400}, "authentication": {401}, "billing": {429, 402},
    "permission_denied": {403}, "not_found": {404}, "request_too_large": {413},
    "rate_limited": {429}, "provider_error": {500}, "timeout": {504, 408}, "overloaded": {503},
}


# Kinds a second try of the same request can't change. Errors sent as 408 or 409 keep the SDKs'
# own rule for those statuses.
LASTING = {
    "invalid_request", "context_too_long", "unsupported", "content_filtered", "authentication",
    "billing", "permission_denied", "not_found", "request_too_large",
}


def _counting_client(sdk):
    sent = []
    return sdk.DefaultHttpxClient(event_hooks={"request": [sent.append]}), sent


def _sent_once(info, want, sent):
    if info.value.status_code in (408, 409):
        assert info.value.response.headers.get("x-should-retry") is None, want
        return
    assert info.value.response.headers.get("x-should-retry") == "false", want
    assert len(sent) == 1, want


def test_anthropic_sdk_sends_a_lasting_error_once(base_url, scenario):
    want = expected(base_url, scenario)
    if want["error_kind"] not in LASTING:
        pytest.skip("a retry may pass")
    http, sent = _counting_client(anthropic)
    c = anthropic.Anthropic(base_url=base_url, api_key="test", http_client=http)
    with pytest.raises(anthropic.APIStatusError) as info:
        c.messages.create(model=f"{scenario['source']}:{scenario['name']}", max_tokens=8, messages=MESSAGES)
    _sent_once(info, want, sent)


@pytest.mark.parametrize("endpoint", ["chat", "responses"])
def test_openai_sdk_sends_a_lasting_error_once(base_url, scenario, endpoint):
    want = expected(base_url, scenario)
    if want["error_kind"] not in LASTING:
        pytest.skip("a retry may pass")
    http, sent = _counting_client(openai)
    c = openai.OpenAI(base_url=base_url + "/v1", api_key="test", http_client=http)
    model = f"{scenario['source']}:{scenario['name']}"
    with pytest.raises(openai.APIStatusError) as info:
        if endpoint == "chat":
            c.chat.completions.create(model=model, messages=MESSAGES)
        else:
            c.responses.create(model=model, input="hi")
    _sent_once(info, want, sent)


def test_anthropic_endpoint(base_url, scenario):
    want = expected(base_url, scenario)
    c = anthropic.Anthropic(base_url=base_url, api_key="test", max_retries=0)
    with pytest.raises(anthropic.APIStatusError) as info:
        c.messages.create(model=f"{scenario['source']}:{scenario['name']}", max_tokens=8, messages=MESSAGES)
    allowed = set(ANTHROPIC_STATUS[want["error_kind"]])
    if scenario["source"] == "anthropic_messages":
        # An Anthropic error reaches an Anthropic app as sent, even when Kaeso reads its kind
        # from the message, as for the 400 Anthropic answers an empty credit balance with.
        allowed.add(want["status"])
    assert info.value.status_code in allowed, want
    body = info.value.body
    assert isinstance(body, dict) and body.get("type") == "error", body
    if want["status"] == 429 and want.get("retry_after"):
        assert info.value.response.headers.get("retry-after"), "retry-after lost"


@pytest.mark.parametrize("endpoint", ["chat", "responses"])
def test_openai_endpoints(base_url, scenario, endpoint):
    want = expected(base_url, scenario)
    c = openai.OpenAI(base_url=base_url + "/v1", api_key="test", max_retries=0)
    model = f"{scenario['source']}:{scenario['name']}"
    with pytest.raises(openai.APIStatusError) as info:
        if endpoint == "chat":
            c.chat.completions.create(model=model, messages=MESSAGES)
        else:
            c.responses.create(model=model, input="hi")
    assert info.value.status_code in OPENAI_STATUS[want["error_kind"]], want
    assert isinstance(info.value.body, dict) and "message" in info.value.body, info.value.body
    if want["status"] == 429 and want.get("retry_after"):
        assert info.value.response.headers.get("retry-after"), "retry-after lost"


def test_gemini_endpoint(base_url, scenario):
    want = expected(base_url, scenario)
    c = genai.Client(api_key="test", http_options=types.HttpOptions(base_url=base_url, api_version="v1beta"))
    with pytest.raises(errors.APIError) as info:
        c.models.generate_content(model=f"{scenario['source']}:{scenario['name']}", contents="hi")
    allowed = set(GEMINI_STATUS[want["error_kind"]])
    if scenario["source"] == "gemini":
        allowed.add(want["status"])
    assert info.value.code in allowed, want
    assert info.value.message, info.value
