from __future__ import annotations

import anthropic
import openai
import pytest

from conftest import expected

SCENARIO_KINDS = ("error",)
SCENARIO_SOURCES = ("anthropic_messages", "openai_chat", "openai_responses", "openai_embeddings")
MESSAGES = [{"role": "user", "content": "hi"}]

ANTHROPIC_STATUS = {
    "invalid_request": {400}, "context_too_long": {400}, "unsupported": {400},
    "content_filtered": {400}, "authentication": {401}, "billing": {402},
    "permission_denied": {403}, "not_found": {404}, "request_too_large": {413},
    "rate_limited": {429}, "provider_error": {500}, "timeout": {504}, "overloaded": {529},
}
OPENAI_STATUS = {
    "invalid_request": {400, 409, 422}, "context_too_long": {400}, "unsupported": {400},
    "content_filtered": {400}, "authentication": {401}, "billing": {429, 402},
    "permission_denied": {403}, "not_found": {404}, "request_too_large": {413},
    "rate_limited": {429}, "provider_error": {500}, "timeout": {504, 408}, "overloaded": {503},
}


def test_anthropic_endpoint(base_url, scenario):
    want = expected(base_url, scenario)
    c = anthropic.Anthropic(base_url=base_url, api_key="test", max_retries=0)
    with pytest.raises(anthropic.APIStatusError) as info:
        c.messages.create(model=f"{scenario['source']}:{scenario['name']}", max_tokens=8, messages=MESSAGES)
    assert info.value.status_code in ANTHROPIC_STATUS[want["error_kind"]], want
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
