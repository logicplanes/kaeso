import pathlib

import pytest
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
