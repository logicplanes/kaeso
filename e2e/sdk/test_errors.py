import time

import anthropic
import openai
import pytest


def user(text: str) -> list:
    return [{"role": "user", "content": text}]


def test_authentication(install):
    bad = openai.OpenAI(base_url=f"{install.url}/v1", api_key="sk-kso-nope", max_retries=0)
    with pytest.raises(openai.AuthenticationError):
        bad.chat.completions.create(model="gpt-4o", messages=user("x"))
    bad = anthropic.Anthropic(base_url=install.url, api_key="sk-kso-nope", max_retries=0)
    with pytest.raises(anthropic.AuthenticationError) as e:
        bad.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("x"))
    assert e.value.body["error"]["type"] == "authentication_error"


def test_permission(install):
    narrow = anthropic.Anthropic(base_url=install.url, api_key=install.narrow_key, max_retries=0)
    with pytest.raises(anthropic.PermissionDeniedError) as e:
        narrow.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("x"))
    assert "key's model list" in str(e.value)
    platform = openai.OpenAI(base_url=f"{install.url}/v1", api_key=install.platform_key, max_retries=0)
    with pytest.raises(openai.PermissionDeniedError):
        platform.chat.completions.create(model="gpt-4o", messages=user("x"))


def test_not_found(oa, an):
    with pytest.raises(openai.NotFoundError):
        oa.chat.completions.create(model="gpt-9", messages=user("x"))
    with pytest.raises(anthropic.NotFoundError):
        an.messages.create(model="claude-opus-9", max_tokens=10, messages=user("x"))


def test_refused_by_kpp_before_sending(an, mock):
    with pytest.raises(anthropic.BadRequestError) as e:
        an.messages.create(model="gpt-4o", max_tokens=10, messages=user("x"), extra_body={"top_k": 5})
    assert "top_k" in str(e.value)
    assert mock.received() == []


def test_provider_4xx_and_5xx(oa, an, mock):
    with pytest.raises(anthropic.BadRequestError):
        an.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("[mock:error=invalid-request-prefill]"))
    with pytest.raises(openai.BadRequestError):
        oa.chat.completions.create(model="gpt-4o", messages=user("[mock:error=context-length-exceeded-400]"))
    with pytest.raises(anthropic.RateLimitError) as e:
        an.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("[mock:error=rate-limit]"))
    assert e.value.response.headers["retry-after"] == "20"
    with pytest.raises(anthropic.APIStatusError) as e:
        an.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("[mock:error=overloaded]"))
    assert e.value.status_code == 529
    with pytest.raises(openai.InternalServerError):
        oa.chat.completions.create(model="claude-haiku-4-5", messages=user("[mock:error=overloaded]"))


def test_timeout(oa):
    start = time.monotonic()
    with pytest.raises(openai.InternalServerError) as e:
        oa.chat.completions.create(model="gpt-4o-mini", messages=user("[mock:stall]"))
    assert e.value.status_code == 504
    assert time.monotonic() - start < 10


def test_sdk_retries(install, mock):
    retrying = anthropic.Anthropic(base_url=install.url, api_key=install.key, max_retries=2)
    with pytest.raises(anthropic.APIStatusError):
        retrying.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("[mock:error=overloaded]"))
    assert len(mock.received()) == 3, "Kaeso does not retry; the SDK retried twice"
    mock.reset()
    with pytest.raises(anthropic.BadRequestError):
        retrying.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("[mock:error=invalid-request-prefill]"))
    assert len(mock.received()) == 1, "no retries on a 400"


def test_disconnect_cancels_the_provider_stream(an, mock):
    with an.messages.stream(model="claude-haiku-4-5", max_tokens=100, messages=user("Hello [mock:slow]")) as s:
        for _ in s.text_stream:
            break
    for _ in range(100):
        if mock.streams()["dropped"] == 1:
            break
        time.sleep(0.05)
    assert mock.streams()["dropped"] == 1


def test_limits(install, mock):
    key = install.post(
        f"{install.base}/keys",
        {"name": "limited", "limits": [{"kind": "request_rate", "period": "minute", "amount": 1}]},
    )["secret"]
    client = openai.OpenAI(base_url=f"{install.url}/v1", api_key=key, max_retries=0)
    client.chat.completions.create(model="gpt-4o", messages=user("x"))
    with pytest.raises(openai.RateLimitError) as e:
        client.chat.completions.create(model="gpt-4o", messages=user("x"))
    assert "request rate of 1 requests per minute" in str(e.value)
    assert 1 <= int(e.value.response.headers["retry-after"]) <= 60
    anth = anthropic.Anthropic(base_url=install.url, api_key=key, max_retries=0)
    with pytest.raises(anthropic.RateLimitError) as e:
        anth.messages.create(model="claude-haiku-4-5", max_tokens=10, messages=user("x"))
    assert e.value.body["error"]["type"] == "rate_limit_error"
