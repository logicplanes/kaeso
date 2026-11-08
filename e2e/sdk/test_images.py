import base64

import openai
import pytest

PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=="
)


def test_generate_with_a_gpt_image_model(oa, mock):
    image = oa.images.generate(model="gpt-image-2", prompt="A lighthouse at dusk", size="1024x1024", quality="high")
    assert base64.b64decode(image.data[0].b64_json).startswith(b"\x89PNG")
    assert image.usage.input_tokens_details.image_tokens == 40
    sent = mock.last()
    assert sent["path"] == "/v1/images/generations"
    assert sent["body"]["quality"] == "high"


def test_streamed_generation_brings_partial_and_finished_images(oa, mock):
    events = list(oa.images.generate(model="gpt-image-2", prompt="A red fox", stream=True, partial_images=1))
    assert [e.type for e in events] == ["image_generation.partial_image", "image_generation.completed"]
    assert events[-1].usage.total_tokens == 100


def test_edit_uploads_images_and_a_mask(oa, mock):
    image = oa.images.edit(
        model="gpt-image-2",
        image=[("room.png", PNG, "image/png"), ("lamp.png", PNG, "image/png")],
        mask=("mask.png", PNG, "image/png"),
        prompt="Add the lamp next to the sofa",
    )
    assert image.data[0].b64_json
    sent = mock.last()
    assert sent["path"] == "/v1/images/edits"
    assert sent["headers"]["content-type"].startswith("multipart/form-data; boundary=")
    assert sent["body"].count('filename="') == 3


def test_a_gemini_image_model_serves_an_openai_app(oa, mock):
    image = oa.images.generate(
        model="gemini-3.1-flash-image", prompt="A red circle [mock:fixture=image-output]", size="1024x1024"
    )
    assert image.data[0].b64_json
    assert image.usage.output_tokens_details.image_tokens == 1290
    sent = mock.last()
    assert sent["path"] == "/v1beta/models/gemini-3.1-flash-image:generateContent"
    assert sent["body"]["generationConfig"]["responseModalities"] == ["IMAGE"]
    events = list(
        oa.images.generate(model="gemini-3.1-flash-image", prompt="A red circle [mock:fixture=image-output]", stream=True)
    )
    assert [e.type for e in events] == ["image_generation.completed"]


def test_what_cannot_be_served_is_refused_in_openai_s_shape(oa, mock):
    with pytest.raises(openai.BadRequestError) as info:
        oa.images.create_variation(model="gpt-image-2", image=("a.png", PNG, "image/png"))
    assert "/v1/images/edits" in info.value.message
    with pytest.raises(openai.BadRequestError) as info:
        oa.images.generate(model="claude-haiku-4-5", prompt="A fox")
    assert "image API" in info.value.message
    with pytest.raises(openai.BadRequestError) as info:
        oa.images.generate(model="gemini-3.1-flash-image", prompt="A fox", size="1536x1024")
    assert info.value.param == "size"
    assert mock.received() == []


def test_responses_keep_openai_s_image_generation_tool(oa, mock):
    r = oa.responses.create(model="gpt-4o", input="Draw a fox", tools=[{"type": "image_generation", "quality": "low"}])
    assert r.output_text
    assert mock.last()["body"]["tools"] == [{"type": "image_generation", "quality": "low"}]
    with pytest.raises(openai.BadRequestError):
        oa.responses.create(model="claude-haiku-4-5", input="Draw a fox", tools=[{"type": "image_generation"}])
