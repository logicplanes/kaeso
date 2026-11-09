import base64

import httpx
import openai
import pytest

PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg=="
)


def test_a_spacex_video_is_made_polled_and_downloaded(oa, mock, install):
    video = oa.videos.create_and_poll(
        model="grok-imagine-video-1.5",
        prompt="A lighthouse at dusk, waves rolling in",
        seconds="8",
        size="1280x720",
        poll_interval_ms=10,
    )
    assert video.status == "completed"
    assert video.id.startswith("video_")
    assert video.model == "grok-imagine-video-1.5"
    started = next(r for r in mock.received() if r["path"] == "/v1/videos/generations")
    assert started["body"]["duration"] == 8
    assert (started["body"]["resolution"], started["body"]["aspect_ratio"]) == ("720p", "16:9")

    file = oa.videos.download_content(video.id)
    assert file.content[4:8] == b"ftyp"
    part = httpx.get(
        f"{install.url}/v1/videos/{video.id}/content",
        headers={"authorization": f"Bearer {install.key}", "range": "bytes=0-7"},
    )
    assert part.status_code == 206
    assert part.content == file.content[:8]


def test_an_uploaded_first_frame_reaches_spacex(oa, mock):
    video = oa.videos.create(
        model="grok-imagine-video-1.5",
        prompt="The lake ripples",
        input_reference=("first.png", PNG, "image/png"),
    )
    assert video.status in ("queued", "in_progress")
    sent = mock.last()
    assert sent["path"] == "/v1/videos/generations"
    assert sent["body"]["image"]["url"].startswith("data:image/png;base64,")


def test_videos_are_listed_remixed_and_deleted(oa, mock):
    video = oa.videos.create_and_poll(model="grok-imagine-video-1.5", prompt="A fox", poll_interval_ms=10)
    remix = oa.videos.remix(video.id, prompt="The same fox in snow")
    assert remix.remixed_from_video_id == video.id
    assert next(r for r in mock.received() if r["path"] == "/v1/videos/edits")["body"]["prompt"] == "The same fox in snow"

    listed = [v.id for v in oa.videos.list(limit=100)]
    assert video.id in listed and remix.id in listed
    assert listed.index(remix.id) < listed.index(video.id)

    assert oa.videos.delete(remix.id).deleted
    with pytest.raises(openai.NotFoundError):
        oa.videos.retrieve(remix.id)


def test_a_hailuo_video_is_downloaded_through_its_file_id(oa, mock):
    video = oa.videos.create_and_poll(
        model="MiniMax-Hailuo-2.3", prompt="A paper plane over a city", seconds="6", poll_interval_ms=10
    )
    assert video.status == "completed"
    assert oa.videos.download_content(video.id).content[4:8] == b"ftyp"
    assert any(r["path"] == "/v1/files/retrieve" for r in mock.received())


def test_what_cannot_be_served_is_refused_in_openai_s_shape(oa, mock):
    with pytest.raises(openai.BadRequestError) as info:
        oa.videos.create(model="sora-2", prompt="A fox")
    assert info.value.param == "model"
    with pytest.raises(openai.BadRequestError):
        oa.videos.create(model="claude-haiku-4-5", prompt="A fox")
    with pytest.raises(openai.NotFoundError):
        oa.videos.edit(prompt="A fox", video=("clip.mp4", b"\x00", "video/mp4"))
    with pytest.raises(openai.NotFoundError):
        oa.videos.retrieve("video_06a1aaaaaaaaaaaaaaaaaaaaaa")
    assert mock.received() == []
