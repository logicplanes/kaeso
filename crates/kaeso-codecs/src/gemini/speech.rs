//! `generationConfig.responseModalities`, `.speechConfig` and the audio of `.responseFormat`
//! (discovery document `GenerationConfig`, `SpeechConfig`, `AudioResponseFormat`; speech
//! generation guide, "Audio output formats").

use kaeso_kpp::{AudioFormat, AudioOutput, Extras, GenerateRequest, KppError, Modality, Origin};
use serde_json::{Map, Value, json};

use super::D;
use super::wire::{PrebuiltVoiceW, Raw, SpeechConfigW, VoiceConfigW};
use crate::cross::{Setting, app_field};
use crate::wire::{
    Ext, add_nested_extra, nested_extra_for, verbatim_from_form, verbatim_into_form,
};

pub(crate) const FORM_MODALITIES_EMPTY: &str = "modalities_empty";
const FORM_SPEECH: &str = "speech_config";
const FORM_VOICE: &str = "voice";
const FORM_VOICE_CONFIG: &str = "voice_config";
const FORM_PREBUILT: &str = "prebuilt";
const FORM_SAMPLE_RATE: &str = "sample_rate";
const SPEECH: &str = "speechConfig";
const VOICE_CONFIG: &str = "speechConfig.voiceConfig";
const PREBUILT: &str = "speechConfig.voiceConfig.prebuiltVoiceConfig";
const PCM_RATE: u64 = 24_000;

fn modality(wire: &str) -> Modality {
    match Modality::from_wire(&wire.to_ascii_lowercase()) {
        Modality::Other(_) => Modality::Other(wire.to_owned()),
        typed => typed,
    }
}

fn wire_modality(m: &Modality) -> String {
    match m {
        Modality::Other(s) => s.clone(),
        typed => typed.as_wire().to_ascii_uppercase(),
    }
}

pub(crate) fn decode(
    req: &mut GenerateRequest,
    origin: &Origin,
    modalities: Option<Vec<String>>,
    speech: Option<Ext<SpeechConfigW>>,
    format: Option<Raw>,
) -> Option<Raw> {
    if let Some(m) = modalities {
        if m.is_empty() {
            req.extras
                .form
                .insert(FORM_MODALITIES_EMPTY.into(), true.into());
        }
        req.output_modalities = m.iter().map(|s| modality(s)).collect();
    }
    let mut audio = AudioOutput::default();
    audio.extras = Extras::new(origin.clone());
    let mut used = false;
    if let Some(sc) = speech {
        used = true;
        decode_speech(sc, &mut audio);
    }
    let rest = match format.as_ref().and_then(audio_format) {
        Some((f, rate)) => {
            used = true;
            audio.format = Some(f);
            if rate {
                audio
                    .extras
                    .form
                    .insert(FORM_SAMPLE_RATE.into(), PCM_RATE.into());
            }
            None
        }
        None => format,
    };
    if used {
        req.audio_output = Some(audio);
    }
    rest
}

fn decode_speech(sc: Ext<SpeechConfigW>, audio: &mut AudioOutput) {
    let e = &mut audio.extras;
    e.form.insert(FORM_SPEECH.into(), true.into());
    add_nested_extra(e, SPEECH, sc.extra);
    verbatim_into_form(e, SPEECH, sc.verbatim);
    let Some(vc) = sc.known.voice_config else {
        return;
    };
    e.form.insert(FORM_VOICE_CONFIG.into(), true.into());
    add_nested_extra(e, VOICE_CONFIG, vc.extra);
    verbatim_into_form(e, VOICE_CONFIG, vc.verbatim);
    let VoiceConfigW {
        prebuilt_voice_config,
        voice,
    } = vc.known;
    if let Some(p) = prebuilt_voice_config {
        e.form.insert(FORM_PREBUILT.into(), true.into());
        add_nested_extra(e, PREBUILT, p.extra);
        verbatim_into_form(e, PREBUILT, p.verbatim);
        audio.voice = p.known.voice_name;
        if let Some(v) = voice
            && let Ok(raw) = kaeso_kpp::RawJson::from_serialize(&v)
        {
            audio
                .extras
                .fields
                .insert(format!("{VOICE_CONFIG}.voice"), raw);
        }
    } else if let Some(v) = voice {
        e.form.insert(FORM_VOICE.into(), "voice".into());
        audio.voice = Some(v);
    }
}

// Only an audio format that names one KPP format is typed; anything else, such as μ-law, another
// sample rate or an image format beside it, stays raw.
fn audio_format(raw: &Raw) -> Option<(AudioFormat, bool)> {
    let Ok(Value::Object(top)) = serde_json::from_str::<Value>(raw.get()) else {
        return None;
    };
    let [(key, Value::Object(audio))] = top.iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    if key != "audio" || audio.keys().any(|k| k != "mimeType" && k != "sampleRate") {
        return None;
    }
    let rate = audio.get("sampleRate").map(Value::as_u64);
    let format = match audio.get("mimeType").and_then(Value::as_str)? {
        "AUDIO_WAV" if rate.is_none() => AudioFormat::Wav,
        "AUDIO_MP3" if rate.is_none() => AudioFormat::Mp3,
        "AUDIO_L16" if matches!(rate, None | Some(Some(PCM_RATE))) => AudioFormat::Pcm16,
        _ => return None,
    };
    Some((format, rate.is_some()))
}

fn wire_format(f: &AudioFormat) -> Option<&'static str> {
    match f {
        AudioFormat::Wav => Some("AUDIO_WAV"),
        AudioFormat::Mp3 => Some("AUDIO_MP3"),
        AudioFormat::Pcm16 => Some("AUDIO_L16"),
        _ => None,
    }
}

pub(crate) struct Encoded {
    pub(crate) modalities: Option<Vec<String>>,
    pub(crate) speech: Option<Ext<SpeechConfigW>>,
    pub(crate) format: Option<Raw>,
}

// Chat Completions asks for audio as `["text", "audio"]`, its text being the audio's transcript;
// Gemini's speech models answer only an exact `["AUDIO"]` ("an exact match to the modalities of
// the response").
pub(crate) fn encode(r: &GenerateRequest, own: bool) -> Result<Encoded, KppError> {
    let modalities = if own {
        (!r.output_modalities.is_empty() || r.extras.form_flag(D, FORM_MODALITIES_EMPTY))
            .then(|| r.output_modalities.iter().map(wire_modality).collect())
    } else if r.output_modalities.contains(&Modality::Audio) {
        Some(vec!["AUDIO".to_owned()])
    } else if r.output_modalities.is_empty() {
        None
    } else {
        Some(r.output_modalities.iter().map(wire_modality).collect())
    };
    let Some(audio) = &r.audio_output else {
        return Ok(Encoded {
            modalities,
            speech: None,
            format: None,
        });
    };
    let own_audio = own && audio.extras.is_from(D);
    let speech = if own_audio {
        audio
            .extras
            .form_flag(D, FORM_SPEECH)
            .then(|| own_speech(audio))
    } else {
        audio.voice.as_ref().map(|v| {
            Ext::new(SpeechConfigW {
                voice_config: Some(Ext::new(VoiceConfigW {
                    prebuilt_voice_config: Some(Ext::new(PrebuiltVoiceW {
                        voice_name: Some(v.clone()),
                    })),
                    voice: None,
                })),
            })
        })
    };
    let format = match audio.format.as_ref().and_then(wire_format) {
        Some(mime) => {
            let mut a = Map::new();
            a.insert("mimeType".into(), mime.into());
            if own_audio && let Some(rate) = audio.extras.form_value(D, FORM_SAMPLE_RATE) {
                a.insert("sampleRate".into(), rate.clone());
            }
            Some(
                serde_json::value::to_raw_value(&json!({ "audio": a })).map_err(|e| {
                    KppError::provider(format!("Kaeso could not write `responseFormat` ({e})."))
                })?,
            )
        }
        None => None,
    };
    Ok(Encoded {
        modalities,
        speech,
        format,
    })
}

fn own_speech(audio: &AudioOutput) -> Ext<SpeechConfigW> {
    let e = &audio.extras;
    let voice_config = e.form_flag(D, FORM_VOICE_CONFIG).then(|| {
        let as_voice = e.form_str(D, FORM_VOICE) == Some("voice");
        let prebuilt = (e.form_flag(D, FORM_PREBUILT) || (!as_voice && audio.voice.is_some()))
            .then(|| {
                Ext::with(
                    PrebuiltVoiceW {
                        voice_name: audio.voice.clone().filter(|_| !as_voice),
                    },
                    nested_extra_for(e, D, PREBUILT),
                    verbatim_from_form(e, D, PREBUILT),
                )
            });
        let mut extra = nested_extra_for(e, D, VOICE_CONFIG);
        let voice = if as_voice {
            audio.voice.clone()
        } else {
            extra
                .shift_remove("voice")
                .and_then(|v| v.parse::<String>().ok())
        };
        Ext::with(
            VoiceConfigW {
                prebuilt_voice_config: prebuilt,
                voice,
            },
            extra,
            verbatim_from_form(e, D, VOICE_CONFIG),
        )
    });
    Ext::with(
        SpeechConfigW { voice_config },
        nested_extra_for(e, D, SPEECH),
        verbatim_from_form(e, D, SPEECH),
    )
}

pub(crate) fn check(r: &GenerateRequest) -> Result<(), KppError> {
    let refuse = |feature: String, hint: &str, setting: Setting| {
        Err(KppError::unsupported_by(feature, D, hint).with_param(app_field(r, setting, D)))
    };
    if let Some(Modality::Other(m)) = r
        .output_modalities
        .iter()
        .find(|m| matches!(m, Modality::Other(_)))
    {
        return refuse(
            format!("output modality `{m}`"),
            "Gemini returns text, images and audio.",
            Setting::Modalities,
        );
    }
    let Some(audio) = &r.audio_output else {
        return Ok(());
    };
    if let Some(v) = &audio.voice
        && !crate::media::has_voice(crate::media::GEMINI_VOICES, v)
    {
        return refuse(
            format!("voice `{v}`"),
            "Gemini's prebuilt voices are Zephyr, Puck, Charon, Kore and 26 more; name one of them.",
            Setting::Voice,
        );
    }
    if let Some(f) = &audio.format
        && wire_format(f).is_none()
    {
        return refuse(
            format!("audio format `{}`", f.as_wire()),
            "Gemini writes audio as wav, mp3 or pcm16.",
            Setting::AudioFormat,
        );
    }
    Ok(())
}
