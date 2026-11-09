from __future__ import annotations

import warnings

import openai
import pytest

from conftest import FIXTURES, expected

SCENARIO_KINDS = ("video",)
SCENARIO_SOURCES = (
    "openai_videos",
    "xai_videos",
    "minimax_videos",
    "zai_videos",
    "openrouter_videos",
    "veo",
    "gemini_interactions",
    "elevenlabs",
)

PNG = (FIXTURES / "openai_videos" / "requests" / "create-upload.multipart").read_bytes().split(b"\r\n\r\n")[-1].split(b"\r\n--")[0]

# The SDK marks its video calls deprecated since OpenAI shut Sora down; Kaeso serves the API for
# other providers' models.
pytestmark = pytest.mark.filterwarnings("ignore::DeprecationWarning")


def client(base_url: str) -> openai.OpenAI:
    return openai.OpenAI(base_url=base_url + "/v1", api_key="test", max_retries=0)


def test_create_and_retrieve(base_url, scenario):
    want = expected(base_url, scenario)
    assert "error" not in want, want
    model = f"{scenario['source']}:{scenario['name']}"
    c = client(base_url)
    video = c.videos.create(model=model, prompt="A red fox in snow", seconds="8", size="1280x720")
    assert video.object == "video"
    assert video.id == model
    assert video.status == want["status"]
    assert video.progress == want["progress"]
    if want["seconds"]:
        assert video.seconds == want["seconds"]
    if want["error_code"]:
        assert video.error is not None and video.error.code == want["error_code"]
    again = c.videos.retrieve(video.id)
    assert again.status == video.status


def test_upload_remix_list_download_and_delete(base_url):
    c = client(base_url)
    created = c.videos.create(
        model="xai_videos:polls/done",
        prompt="The lake ripples",
        input_reference=("first.png", PNG, "image/png"),
    )
    assert created.status == "completed"
    remix = c.videos.remix(created.id, prompt="Make it snow")
    assert remix.status == "completed"
    page = c.videos.list(limit=2)
    assert page.data and len(page.data) <= 2
    assert all(v.object == "video" for v in page.data)
    content = c.videos.download_content(created.id)
    assert content.read()[:8] == (FIXTURES / "openai_videos" / "content" / "sample.mp4").read_bytes()[:8]
    deleted = c.videos.delete(created.id)
    assert deleted.deleted and deleted.id == created.id


def test_a_finished_job_polls_to_its_end(base_url):
    c = client(base_url)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        done = c.videos.poll("minimax_videos:polls/h3-succeeded", poll_interval_ms=1)
    assert done.status == "completed"
    assert done.seconds == "5"
