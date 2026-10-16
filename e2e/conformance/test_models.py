from __future__ import annotations

from datetime import datetime, timezone

import anthropic
import openai
import pytest

VISIBLE = ["claude-opus-5", "claude-sonnet-5", "gpt-6-astra", "claude-sonnet-4-5-20250929"]
RETIRED = "claude-3-5-sonnet-20241022"


def utc(y: int, m: int, d: int) -> datetime:
    return datetime(y, m, d, tzinfo=timezone.utc)


def claude(url: str) -> anthropic.Anthropic:
    return anthropic.Anthropic(base_url=url, api_key="test", max_retries=0)


def gpt(url: str) -> openai.OpenAI:
    return openai.OpenAI(base_url=url + "/v1", api_key="test", max_retries=0)


def test_anthropic_list_pages_through_every_model(base_url):
    c = claude(base_url)
    first = c.models.list(limit=2)
    assert [m.id for m in first.data] == VISIBLE[:2]
    assert first.has_more is True
    assert (first.first_id, first.last_id) == (VISIBLE[0], VISIBLE[1])
    assert [m.id for m in c.models.list(limit=2)] == VISIBLE
    whole = c.models.list()
    assert [m.id for m in whole.data] == VISIBLE and whole.has_more is False
    opus = whole.data[0]
    assert opus.type == "model"
    assert opus.display_name == "Claude Opus 5"
    assert opus.created_at == utc(2026, 7, 24)
    assert opus.lifecycle == "active"
    assert (opus.max_input_tokens, opus.max_tokens) == (1_000_000, 128_000)
    assert opus.capabilities is None and opus.line is None


def test_anthropic_list_filters_and_pages_backwards(base_url):
    c = claude(base_url)
    retired = c.models.list(lifecycle=["retired"])
    assert [m.id for m in retired.data] == [RETIRED]
    both = c.models.list(lifecycle=["active", "retired"])
    assert [m.id for m in both.data] == VISIBLE[:3] + [RETIRED]
    before = c.models.list(before_id=VISIBLE[3], limit=2)
    assert [m.id for m in before.data] == VISIBLE[1:3] and before.has_more is True


def test_anthropic_auto_pages_backwards(base_url):
    c = claude(base_url)
    got = [m.id for m in c.models.list(before_id=VISIBLE[3], limit=1)]
    assert got == [VISIBLE[2], VISIBLE[1], VISIBLE[0]]
    got = [m.id for m in c.models.list(before_id=VISIBLE[3], limit=2)]
    assert got == [VISIBLE[1], VISIBLE[2], VISIBLE[0]]


def test_anthropic_unknown_cursor(base_url):
    with pytest.raises(anthropic.BadRequestError) as info:
        claude(base_url).models.list(after_id="claude-nonexistent")
    assert "after_id" in info.value.body["error"]["message"]


def test_anthropic_retrieve(base_url):
    m = claude(base_url).models.retrieve("claude-sonnet-4-5-20250929")
    assert m.display_name == "Claude Sonnet 4.5"
    assert m.lifecycle == "deprecated"
    assert m.deprecated_at == utc(2026, 6, 10)
    assert m.retires_at == utc(2026, 12, 10)
    assert claude(base_url).models.retrieve(RETIRED).lifecycle == "retired"


def test_anthropic_unknown_model_and_bad_limit(base_url):
    c = claude(base_url)
    with pytest.raises(anthropic.NotFoundError) as info:
        c.models.retrieve("claude-nonexistent")
    assert info.value.body["error"]["type"] == "not_found_error"
    with pytest.raises(anthropic.BadRequestError) as info:
        c.models.list(limit=0)
    assert "limit" in info.value.body["error"]["message"]


def test_openai_list(base_url):
    models = list(gpt(base_url).models.list())
    assert [m.id for m in models] == VISIBLE
    m = models[2]
    assert (m.object, m.owned_by, m.created) == ("model", "openai", 1_777_593_600)
    assert m.shutdown_date is None
    assert str(models[3].shutdown_date) == "2026-12-10"


def test_openai_retrieve(base_url):
    m = gpt(base_url).models.retrieve("claude-opus-5")
    assert (m.id, m.object, m.owned_by, m.created) == ("claude-opus-5", "model", "anthropic", 1_784_851_200)


def test_openai_unknown_model(base_url):
    with pytest.raises(openai.NotFoundError):
        gpt(base_url).models.retrieve("gpt-nonexistent")
