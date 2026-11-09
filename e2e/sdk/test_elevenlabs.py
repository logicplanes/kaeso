import asyncio
import base64
import pathlib

import pytest
from elevenlabs import AudioFormat, CommitStrategy, RealtimeEvents
from elevenlabs.core.api_error import ApiError

FIXTURES = pathlib.Path(__file__).resolve().parents[2] / "crates" / "kaeso-codecs" / "fixtures" / "elevenlabs"
VOICE = "hpp4J3VqNfWAUOO0d1Us"
MP3 = (FIXTURES / "responses" / "recorded-speech.mp3").read_bytes()


def test_text_to_speech_plain_streamed_and_timed(el, mock):
    audio = b"".join(el.text_to_speech.convert(voice_id=VOICE, text="Hi.", model_id="eleven_flash_v2_5"))
    assert audio == MP3
    assert mock.last()["path"] == f"/v1/text-to-speech/{VOICE}"
    streamed = b"".join(el.text_to_speech.stream(voice_id=VOICE, text="Hi.", model_id="eleven_flash_v2_5", output_format="pcm_24000"))
    assert len(streamed) > 0
    timed = el.text_to_speech.convert_with_timestamps(voice_id=VOICE, text="Hi.", model_id="eleven_flash_v2_5")
    assert timed.alignment.characters == ["H", "i", "."]
    pieces = list(el.text_to_speech.stream_with_timestamps(voice_id=VOICE, text="Hi.", model_id="eleven_flash_v2_5"))
    assert len(pieces) == 2 and pieces[0].audio_base_64


def test_speech_to_text_sound_effects_and_dialogue(el, mock):
    t = el.speech_to_text.convert(model_id="scribe_v2", file=("hello.mp3", MP3, "audio/mpeg"))
    assert t.text == "Hello, Kizo"
    sound = b"".join(el.text_to_sound_effects.convert(text="A short click.", duration_seconds=0.5, model_id="eleven_text_to_sound_v2"))
    assert len(sound) > 0
    dialogue = b"".join(
        el.text_to_dialogue.convert(inputs=[{"text": "Hi.", "voice_id": VOICE}], model_id="eleven_flash_v2_5")
    )
    assert len(dialogue) > 0


def test_listings(el, mock):
    models = el.models.list()
    assert "eleven_flash_v2_5" in [m.model_id for m in models]
    voices = el.voices.search(page_size=3)
    assert voices.voices[0].voice_id == VOICE


def test_management_is_refused_in_elevenlabs_shape(el, mock):
    with pytest.raises(ApiError) as e:
        el.user.subscription.get()
    assert e.value.status_code == 404
    assert "ElevenLabs" in e.value.body["detail"]["message"]


def plain_sockets(el):
    # The SDK opens text to speech sockets on wss:// whatever the base URL's scheme
    # (elevenlabs/realtime_tts.py, RealtimeTextToSpeechClient.__init__); the test gateway is plain http.
    el.text_to_speech._ws_base_url = el._client_wrapper.get_base_url().replace("http://", "ws://", 1)


def test_realtime_text_to_speech_streams_through_kaeso(el, mock):
    plain_sockets(el)
    audio = b"".join(
        el.text_to_speech.convert_realtime(voice_id=VOICE, text=iter(["Hello ", "there."]), model_id="eleven_flash_v2_5")
    )
    assert audio == b"mock-audio" * 2
    sent = [r for r in mock.received() if r["path"] == f"/v1/text-to-speech/{VOICE}/stream-input"]
    assert sent[0]["method"] == "GET" and sent[0]["headers"]["xi-api-key"].startswith("sk_test-elevenlabs")
    assert all("sk-kso" not in str(r["body"]) for r in sent)


def test_realtime_speech_to_text_commits_a_transcript(el, mock):
    async def session():
        connection = await el.speech_to_text.realtime.connect(
            {
                "model_id": "scribe_v2_realtime",
                "audio_format": AudioFormat.PCM_16000,
                "sample_rate": 16000,
                "commit_strategy": CommitStrategy.MANUAL,
            }
        )
        committed = asyncio.get_running_loop().create_future()
        connection.on(
            RealtimeEvents.COMMITTED_TRANSCRIPT,
            lambda data: committed.done() or committed.set_result(data),
        )
        await connection.send({"audio_base_64": base64.b64encode(bytes(3200)).decode()})
        await connection.commit()
        data = await asyncio.wait_for(committed, timeout=10)
        await connection.close()
        return data

    assert asyncio.run(session())["text"] == "Hello from the mock."


def test_realtime_sockets_refuse_what_kaeso_cannot_open(el, mock):
    plain_sockets(el)
    with pytest.raises(Exception) as e:
        b"".join(el.text_to_speech.convert_realtime(voice_id=VOICE, text=iter(["Hi."])))
    assert "model_id" in str(e.value) or "400" in str(e.value)
