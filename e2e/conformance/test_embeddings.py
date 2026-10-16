from __future__ import annotations

import openai
import pytest

from conftest import expected

SCENARIO_KINDS = ("embed",)
SCENARIO_SOURCES = ("openai_embeddings",)


def test_embeddings(base_url, scenario):
    want = expected(base_url, scenario)
    c = openai.OpenAI(base_url=base_url + "/v1", api_key="test", max_retries=0)
    model = f"{scenario['source']}:{scenario['name']}"
    if "error" in want:
        with pytest.raises(openai.APIStatusError) as info:
            c.embeddings.create(model=model, input="hello")
        assert info.value.body["message"] == want["error"]
        return
    got = c.embeddings.create(model=model, input="hello")
    assert len(got.data) == want["count"]
    if want["dims"]:
        assert len(got.data[0].embedding) == want["dims"]
