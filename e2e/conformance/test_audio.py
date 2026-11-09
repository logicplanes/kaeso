from __future__ import annotations

import pathlib

import openai

from conftest import FIXTURES

MP3 = (FIXTURES / "openai_speech" / "responses" / "speech.mp3").read_bytes()


def client(base_url: str) -> openai.OpenAI:
    return openai.OpenAI(base_url=base_url + "/v1", api_key="test", max_retries=0)


def test_speech_comes_back_as_the_audio(base_url):
    c = client(base_url)
    with c.audio.speech.with_streaming_response.create(
        model="openai_speech:speech", voice="alloy", input="The quick brown fox."
    ) as response:
        assert response.headers["content-type"] == "audio/mpeg"
        assert response.read() == MP3


def test_speech_events_carry_the_audio_and_the_usage(base_url):
    c = client(base_url)
    with c.audio.speech.with_streaming_response.create(
        model="openai_speech:stream-speech", voice="alloy", input="Hi", stream_format="sse"
    ) as response:
        text = response.read().decode()
    assert text.count("speech.audio.delta") == 2
    assert '"total_tokens":115' in text


def transcribe(base_url: str, model: str, **kwargs):
    return client(base_url).audio.transcriptions.create(
        model=model, file=("speech.mp3", MP3, "audio/mpeg"), **kwargs
    )


def test_every_transcript_form(base_url):
    j = transcribe(base_url, "openai_transcriptions:json")
    assert j.text.startswith("Imagine") and j.usage.input_token_details.audio_tokens == 14
    v = transcribe(base_url, "openai_transcriptions:verbose-segments", response_format="verbose_json")
    assert v.segments[0].id == 0 and v.duration == 3.32
    d = transcribe(base_url, "openai_transcriptions:diarized", response_format="diarized_json")
    assert d.segments[1].speaker == "A"
    srt = transcribe(base_url, "openai_transcriptions:subtitles", response_format="srt")
    assert srt.startswith("1\n00:00:00,000")
    vtt = transcribe(base_url, "openai_transcriptions:subtitles", response_format="vtt")
    assert vtt.startswith("WEBVTT")
    text = transcribe(base_url, "openai_transcriptions:text", response_format="text")
    assert text.strip() == "The beach was a popular spot on a hot summer day."


def test_streamed_transcripts_and_translations(base_url):
    events = list(transcribe(base_url, "openai_transcriptions:stream-transcribe", stream=True))
    assert [e.type for e in events].count("transcript.text.delta") == 6
    assert events[-1].type == "transcript.text.done" and events[-1].text == "I see skies of blue."
    t = client(base_url).audio.translations.create(
        model="openai_transcriptions:translation", file=("speech.mp3", MP3, "audio/mpeg")
    )
    assert t.text.startswith("Hello, my name is Wolfgang")


def test_provider_transcripts_reach_an_openai_app(base_url):
    groq = transcribe(base_url, "openai_transcriptions:providers/groq/verbose", response_format="verbose_json")
    assert groq.segments[0].start == 43.92
    x = transcribe(base_url, "openai_transcriptions:providers/xai/json", response_format="verbose_json")
    assert [w.word for w in x.words] == ["The", "balance"]


def test_dashscope_speech_and_transcripts_reach_an_openai_app(base_url):
    c = client(base_url)
    with c.audio.speech.with_streaming_response.create(
        model="dashscope:speech/qwen-audio-tts", voice="longanhuan_v3.1", input="Hi", response_format="wav"
    ) as response:
        assert response.headers["content-type"] == "audio/wav"
        assert response.read().startswith(b"RIFF")
    with c.audio.speech.with_streaming_response.create(
        model="dashscope:speech/qwen3-tts-pcm", voice="Cherry", input="Hi", response_format="pcm", stream_format="sse"
    ) as response:
        assert response.read().decode().count("speech.audio.delta") == 2
    t = transcribe(base_url, "dashscope:transcriptions/asr")
    assert t.text == "Hello World, this is Alibaba Speech Lab."


def test_errors_raise_the_sdks_errors(base_url):
    import pytest

    with pytest.raises(openai.BadRequestError) as e:
        client(base_url).audio.speech.create(model="openai_speech:errors/invalid-voice-400", voice="alloy", input="Hi")
    assert e.value.body["param"] == "voice"
    with pytest.raises(openai.APIStatusError) as e:
        transcribe(base_url, "openai_transcriptions:errors/file-too-large-413")
    assert e.value.status_code == 413
    assert pathlib.Path(__file__).exists()


def test_elevenlabs_and_minimax_replies_reach_an_openai_app(base_url):
    c = client(base_url)
    with c.audio.speech.with_streaming_response.create(
        model="elevenlabs:recorded-speech", voice="hpp4J3VqNfWAUOO0d1Us", input="Hi"
    ) as response:
        assert response.read().startswith(b"ID3")
    t = transcribe(base_url, "elevenlabs:recorded-transcribe", response_format="verbose_json")
    assert t.text and t.words
