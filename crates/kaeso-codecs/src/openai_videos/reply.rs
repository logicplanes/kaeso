use kaeso_kpp::{
    ImageSize, KppError, VideoError, VideoJob, VideoList, VideoOutput, VideoStatus, VideoVariant,
};
use serde_json::{Map, Value, json};

use super::DIALECT;
use super::profile::VideosBody;
use crate::codec::{EgressCtx, EncodeCtx};
use crate::video_jobs::{Fields, body, provider_object, put_extras};

const SECONDS_NUMBER: &str = "seconds_number";

fn status_of(s: &str, together: bool) -> Option<VideoStatus> {
    match s {
        "queued" if !together => Some(VideoStatus::Queued),
        "in_progress" => Some(VideoStatus::InProgress),
        "completed" => Some(VideoStatus::Completed),
        "failed" => Some(VideoStatus::Failed),
        _ => None,
    }
}

fn unix(f: &mut Fields, key: &str) -> Result<Option<i64>, KppError> {
    match f.take(key) {
        None => Ok(None),
        Some(Value::Number(n)) => Ok(n.as_i64().or_else(|| n.as_f64().map(|x| x.floor() as i64))),
        Some(other) => Err(KppError::provider(format!(
            "`{key}` must be a Unix time, not {other}."
        ))),
    }
}

pub(super) fn decode_job(body: &[u8], ctx: &EgressCtx) -> Result<VideoJob, KppError> {
    let together = ctx.videos_profile.body == VideosBody::Together;
    let mut f = provider_object(body, DIALECT)?;
    let origin = ctx.origin(DIALECT);
    let mut extras = kaeso_kpp::Extras::new(origin.clone());
    let id = f.str("id")?.unwrap_or_default();
    let object = f.str("object")?;
    if object.as_deref().is_some_and(|o| o != "video") {
        return Err(KppError::provider(format!(
            "The provider sent a `{}` object where a video job belongs.",
            object.unwrap_or_default()
        )));
    }
    let status_text = f.str("status")?.unwrap_or_default();
    let status = status_of(&status_text, together).ok_or_else(|| {
        KppError::provider(format!(
            "The provider sent the unknown video status `{status_text}`."
        ))
    })?;
    let mut job = VideoJob::new(id, status);
    job.model = f.str("model")?.unwrap_or_default();
    job.progress = f.u32("progress")?;
    job.created_at = unix(&mut f, "created_at")?;
    job.completed_at = unix(&mut f, "completed_at")?;
    job.expires_at = unix(&mut f, "expires_at")?;
    job.prompt = f.str("prompt")?;
    match f.take("seconds") {
        Some(Value::String(s)) if crate::video_jobs::digits(&s).is_some() => {
            job.seconds = crate::video_jobs::digits(&s);
        }
        Some(Value::Number(n)) if n.as_u64().is_some_and(|n| n <= u64::from(u32::MAX)) => {
            job.seconds = n.as_u64().and_then(|n| u32::try_from(n).ok());
            extras
                .form
                .insert(SECONDS_NUMBER.to_owned(), Value::Bool(true));
        }
        Some(other) => {
            if let Some(raw) = crate::video_jobs::raw(&other) {
                extras.fields.insert("seconds".to_owned(), raw);
            }
        }
        None => {}
    }
    job.size = f
        .str("size")?
        .filter(|s| !s.is_empty())
        .map(|s| ImageSize::from_wire(&s));
    job.remixed_from = f.str("remixed_from_video_id")?;
    if let Some(mut e) = f.object("error")? {
        let mut error = VideoError::new(
            e.str("code")?.unwrap_or_default(),
            e.str("message")?.unwrap_or_default(),
        );
        error.param = e.str("param")?;
        error.kind = e.str("type")?;
        e.rest_into(&mut extras, "error");
        job.error = Some(error);
    }
    if together && let Some(mut outputs) = f.object("outputs")? {
        if let Some(url) = outputs.str("video_url")? {
            job.outputs.push(VideoOutput::new(
                VideoVariant::Video,
                crate::video_jobs::link_source(url),
            ));
        }
        outputs.rest_into(&mut extras, "outputs");
    }
    f.rest_into(&mut extras, "");
    job.extras = extras;
    Ok(job)
}

/// The size an OpenAI `VideoResource` names: the job's, or one its resolution and aspect ratio
/// give, or an empty string when neither is known, since the field is required.
fn size_text(job: &VideoJob) -> String {
    job.size
        .clone()
        .or_else(|| {
            crate::video_jobs::size_of(job.resolution.as_deref()?, job.aspect_ratio.as_deref()?)
        })
        .map(|s| s.as_wire().to_owned())
        .unwrap_or_default()
}

fn error_value(e: &VideoError, job: &VideoJob) -> Value {
    let mut out = Map::new();
    out.insert("code".into(), e.code.clone().into());
    out.insert("message".into(), e.message.clone().into());
    if let Some(p) = &e.param {
        out.insert("param".into(), p.clone().into());
    }
    if let Some(t) = &e.kind {
        out.insert("type".into(), t.clone().into());
    }
    put_extras(&mut out, &job.extras, DIALECT, "error");
    Value::Object(out)
}

fn seconds_value(job: &VideoJob) -> Option<Value> {
    if job.extras.is_from(DIALECT) && job.extras.fields.contains_key("seconds") {
        return None;
    }
    Some(match job.seconds {
        Some(s) if job.extras.form_flag(DIALECT, SECONDS_NUMBER) => json!(s),
        Some(s) => json!(s.to_string()),
        None => json!(""),
    })
}

/// A job as OpenAI's `VideoResource`, every required field written.
pub(super) fn video_value(job: &VideoJob) -> Value {
    let mut out = Map::new();
    out.insert("id".into(), job.id.clone().into());
    out.insert("object".into(), "video".into());
    out.insert("model".into(), job.model.clone().into());
    out.insert("status".into(), job.status.as_str().into());
    let done = u32::from(job.status == VideoStatus::Completed) * 100;
    out.insert("progress".into(), job.progress.unwrap_or(done).into());
    out.insert("created_at".into(), job.created_at.unwrap_or(0).into());
    out.insert("completed_at".into(), json!(job.completed_at));
    out.insert("expires_at".into(), json!(job.expires_at));
    out.insert("prompt".into(), json!(job.prompt));
    out.insert("size".into(), size_text(job).into());
    if let Some(v) = seconds_value(job) {
        out.insert("seconds".into(), v);
    }
    out.insert("remixed_from_video_id".into(), json!(job.remixed_from));
    out.insert(
        "error".into(),
        job.error
            .as_ref()
            .map_or(Value::Null, |e| error_value(e, job)),
    );
    put_extras(&mut out, &job.extras, DIALECT, "");
    Value::Object(out)
}

pub(super) fn encode_video(job: &VideoJob, _ctx: &EncodeCtx) -> Result<Vec<u8>, KppError> {
    match video_value(job) {
        Value::Object(m) => body(&m),
        _ => Ok(b"{}".to_vec()),
    }
}

/// A job as Together AI's `VideoJob`, as the provider writes it.
#[cfg(test)]
pub(super) fn together_value(job: &VideoJob) -> Value {
    use kaeso_kpp::Source;
    let mut out = Map::new();
    out.insert("id".into(), job.id.clone().into());
    out.insert("object".into(), "video".into());
    out.insert("model".into(), job.model.clone().into());
    out.insert("status".into(), job.status.as_str().into());
    out.insert("created_at".into(), job.created_at.unwrap_or(0).into());
    if let Some(t) = job.completed_at {
        out.insert("completed_at".into(), t.into());
    }
    out.insert("size".into(), size_text(job).into());
    if let Some(v) = seconds_value(job) {
        out.insert("seconds".into(), v);
    }
    if let Some(e) = &job.error {
        out.insert("error".into(), error_value(e, job));
    }
    if let Some(VideoOutput {
        source: Source::Url(url),
        ..
    }) = job.output(VideoVariant::Video)
    {
        let mut outputs = Map::new();
        outputs.insert("video_url".into(), url.clone().into());
        put_extras(&mut outputs, &job.extras, DIALECT, "outputs");
        out.insert("outputs".into(), Value::Object(outputs));
    }
    put_extras(&mut out, &job.extras, DIALECT, "");
    Value::Object(out)
}

pub(super) fn encode_list(list: &VideoList) -> Result<Vec<u8>, KppError> {
    let data: Vec<Value> = list.videos.iter().map(video_value).collect();
    let out = json!({
        "object": "list",
        "data": data,
        "first_id": list.videos.first().map(|v| v.id.clone()),
        "last_id": list.videos.last().map(|v| v.id.clone()),
        "has_more": list.has_more,
    });
    crate::wire::to_body(&out)
}

pub(super) fn encode_deleted(id: &str) -> Result<Vec<u8>, KppError> {
    crate::wire::to_body(&json!({ "object": "video.deleted", "deleted": true, "id": id }))
}
