import pathlib

import openai
import pytest

FIXTURES = pathlib.Path(__file__).resolve().parents[2] / "crates" / "kaeso-codecs" / "fixtures"
MP3 = (FIXTURES / "openai_speech" / "responses" / "speech.mp3").read_bytes()


def test_speech_comes_back_as_audio(oa, mock):
    with oa.audio.speech.with_streaming_response.create(model="gpt-4o-mini-tts", voice="coral", input="Hello there.") as r:
        assert r.headers["content-type"] == "audio/mpeg"
        assert r.read() == MP3
    sent = mock.last()
    assert sent["path"] == "/v1/audio/speech"
    assert sent["body"]["stream_format"] == "sse"


def test_speech_events(oa, mock):
    with oa.audio.speech.with_streaming_response.create(
        model="gpt-4o-mini-tts", voice="coral", input="Hello there.", stream_format="sse"
    ) as r:
        text = r.read().decode()
    assert text.count("speech.audio.delta") == 2 and "speech.audio.done" in text


def test_transcriptions_and_translations(oa, mock):
    t = oa.audio.transcriptions.create(model="gpt-4o-transcribe", file=("a.mp3", MP3, "audio/mpeg"))
    assert t.text.startswith("Imagine")
    srt = oa.audio.transcriptions.create(model="whisper-1", file=("a.mp3", MP3, "audio/mpeg"), response_format="srt")
    assert srt.startswith("1\n00:00:00,000")
    events = list(oa.audio.transcriptions.create(model="gpt-4o-transcribe", file=("a.mp3", MP3, "audio/mpeg"), stream=True))
    assert events[-1].type == "transcript.text.done"
    tr = oa.audio.translations.create(model="whisper-1", file=("a.mp3", MP3, "audio/mpeg"))
    assert tr.text.startswith("Hello")
    assert mock.last()["path"] == "/v1/audio/translations"


def test_elevenlabs_models_serve_openai_apps(oa, mock):
    with oa.audio.speech.with_streaming_response.create(
        model="eleven_flash_v2_5", voice="hpp4J3VqNfWAUOO0d1Us", input="Hi.", response_format="pcm"
    ) as r:
        assert r.headers["content-type"] == "audio/pcm"
        assert len(r.read()) > 0
    assert mock.last()["path"] == "/v1/text-to-speech/hpp4J3VqNfWAUOO0d1Us/stream"
    t = oa.audio.transcriptions.create(model="scribe_v2", file=("a.mp3", MP3, "audio/mpeg"), response_format="verbose_json")
    assert t.text == "Hello, Kizo" and len(t.words) == 2


def test_refusals_reach_the_app_before_anything_is_sent(oa, mock):
    mock.reset()
    with pytest.raises(openai.BadRequestError) as e:
        oa.audio.speech.create(model="claude-haiku-4-5", voice="alloy", input="Hi")
    assert "speech" in e.value.message
    with pytest.raises(openai.BadRequestError) as e:
        oa.audio.speech.create(model="eleven_flash_v2_5", voice="alloy", input="Hi")
    assert e.value.body["param"] == "voice"
    assert mock.received() == []
