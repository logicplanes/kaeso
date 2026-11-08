from __future__ import annotations

import base64

import openai
import pytest

from conftest import expected

SCENARIO_KINDS = ("image",)
SCENARIO_SOURCES = ("openai_images", "minimax_images")

PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=="
)


def client(base_url: str) -> openai.OpenAI:
    return openai.OpenAI(base_url=base_url + "/v1", api_key="test", max_retries=0)


def test_generate(base_url, scenario):
    want = expected(base_url, scenario)
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    if scenario["stream"]:
        events = list(c.images.generate(model=model, prompt="A red fox", stream=True, partial_images=1))
        types = [e.type for e in events]
        assert types.count("image_generation.partial_image") == want["partial_images"]
        assert types.count("image_generation.completed") == want["images"]
        assert all(e.b64_json for e in events)
        return
    got = c.images.generate(model=model, prompt="A red fox")
    assert len(got.data) == want["count"]
    assert sum(1 for d in got.data if d.url) == want["links"]
    assert all(d.url or d.b64_json for d in got.data)
    if want["output_tokens"]:
        assert got.usage.output_tokens == want["output_tokens"]


@pytest.mark.parametrize("stream", [False, True])
def test_edit_with_uploads(base_url, stream):
    c = client(base_url)
    files = [("a.png", PNG, "image/png"), ("b.png", PNG, "image/png")]
    if stream:
        events = list(
            c.images.edit(
                model="openai_images:edit-every-field",
                image=files,
                mask=("mask.png", PNG, "image/png"),
                prompt="Put a hat on it",
                stream=True,
                partial_images=1,
            )
        )
        assert [e.type for e in events] == ["image_edit.partial_image", "image_edit.completed"]
        assert events[-1].usage.input_tokens_details.image_tokens == 40
        return
    got = c.images.edit(
        model="openai_images:generate",
        image=files,
        mask=("mask.png", PNG, "image/png"),
        prompt="Put a hat on it",
        input_fidelity="high",
        size="1024x1024",
        n=1,
    )
    assert base64.b64decode(got.data[0].b64_json).startswith(b"\x89PNG")


def test_a_gemini_image_reply_reaches_an_openai_app(base_url):
    got = client(base_url).images.generate(model="gemini:image-output", prompt="A red circle")
    assert len(got.data) == 1 and got.data[0].b64_json
    assert got.usage.output_tokens_details.image_tokens == 1290


def test_variation_upload_is_read(base_url):
    got = client(base_url).images.create_variation(model="openai_images:legacy-dall-e-3-url", image=("a.png", PNG, "image/png"))
    assert got.data[0].url.endswith("otter.png")


def test_provider_errors_keep_openai_s_shape(base_url):
    c = client(base_url)
    with pytest.raises(openai.RateLimitError):
        c.images.generate(model="openai_images:errors/rate-limit-429", prompt="A red fox")
    with pytest.raises(openai.BadRequestError) as info:
        c.images.generate(model="openai_images:errors/moderation-blocked-400", prompt="A red fox")
    assert info.value.code == "moderation_blocked"
    with pytest.raises(openai.BadRequestError) as info:
        c.images.generate(model="minimax_images:errors/sensitive-prompt-200", prompt="A red fox")
