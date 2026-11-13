//! Images in System One requests: the references that stand for them in the state, the
//! instructions and the criteria, the forms each provider takes them in, and the checks of what a
//! provider refuses.

use indexmap::IndexMap;
use kaeso_kpp::{Asset, Extras, KppError, Origin, RawJson, Source};
use serde_json::Value;
use serde_json::value::RawValue;

use super::DIALECT;
use super::profile::SystemOneProfile;
use super::wire::{ImagePartWire, ImageUrlWire};
use crate::image_size::image_info;
use crate::wire::{Ext, entries_of, field_of};

/// Marks an asset Kaeso made from an `image_url` part in the state.
pub(super) const STATE_PART: &str = "state_part";
/// The header of a data URL that isn't `data:<type>;base64`, kept to write it back.
pub(super) const DATA_URL_HEADER: &str = "data_url_header";

pub(super) const IMAGE_PART: &str = "image_url";

/// Calls `replace` for every value of `raw`, outermost first. A value it replaces (`Some`) is not
/// walked into; any other object or array is, and is written again only when something in it was
/// replaced, its keys in their order and every other value byte for byte. `None` when nothing was
/// replaced.
pub(super) fn rewrite(
    raw: &RawValue,
    replace: &mut impl FnMut(&RawValue) -> Result<Option<String>, KppError>,
) -> Result<Option<String>, KppError> {
    if let Some(new) = replace(raw)? {
        return Ok(Some(new));
    }
    let text = raw.get();
    if text.starts_with('{') {
        let Some(entries) = entries_of(raw) else {
            return Ok(None);
        };
        let mut changed = false;
        let mut parts = Vec::with_capacity(entries.len());
        for (key, value) in &entries {
            let new = rewrite(value, replace)?;
            changed |= new.is_some();
            parts.push((key, new));
        }
        if !changed {
            return Ok(None);
        }
        let mut out = String::with_capacity(text.len());
        out.push('{');
        for (i, ((key, new), (_, value))) in parts.iter().zip(&entries).enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&quoted(key));
            out.push(':');
            out.push_str(new.as_deref().unwrap_or(value.get()));
        }
        out.push('}');
        return Ok(Some(out));
    }
    if text.starts_with('[') {
        let Ok(items) = serde_json::from_str::<Vec<&RawValue>>(text) else {
            return Ok(None);
        };
        let mut changed = false;
        let mut news = Vec::with_capacity(items.len());
        for item in &items {
            let new = rewrite(item, replace)?;
            changed |= new.is_some();
            news.push(new);
        }
        if !changed {
            return Ok(None);
        }
        let mut out = String::with_capacity(text.len());
        out.push('[');
        for (i, (new, item)) in news.iter().zip(&items).enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(new.as_deref().unwrap_or(item.get()));
        }
        out.push(']');
        return Ok(Some(out));
    }
    Ok(None)
}

pub(super) fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned())
}

pub(super) fn raw(text: String) -> Result<RawJson, KppError> {
    RawJson::from_string(text).map_err(|e| {
        KppError::provider(format!(
            "Kaeso wrote a System One value that is not JSON ({e}). This is a bug in Kaeso; please report it."
        ))
    })
}

/// Every string in `raw`, without object keys, which references never stand in.
pub(super) fn strings(raw: &RawJson, out: &mut Vec<String>) {
    fn walk(v: Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => out.push(s),
            Value::Array(items) => items.into_iter().for_each(|i| walk(i, out)),
            Value::Object(map) => map.into_iter().for_each(|(_, v)| walk(v, out)),
            _ => {}
        }
    }
    if let Ok(v) = raw.to_value() {
        walk(v, out);
    }
}

/// A reference for a new image that occurs nowhere in `text`, so that no other string of the
/// request can stand for it.
pub(super) fn fresh_reference(text: &str, taken: &IndexMap<String, Asset>, n: usize) -> String {
    let mut i = n;
    loop {
        let candidate = format!("kaeso-image-{i}-ref");
        let clashes = taken
            .keys()
            .any(|k| k.contains(&candidate) || candidate.contains(k.as_str()));
        if !text.contains(&candidate) && !clashes {
            return candidate;
        }
        i += 1;
    }
}

fn is_image_part(raw: &RawValue) -> bool {
    raw.get().starts_with('{')
        && field_of(raw, "type").is_some_and(|t| t.get() == "\"image_url\"")
        && field_of(raw, IMAGE_PART).is_some_and(|v| v.get().starts_with('{'))
}

/// The image parts in an app's state, each replaced by a new reference and added to `assets`;
/// `None` when the state has none. `body` is the request's text, which no new reference occurs in.
pub(super) fn take_state_images(
    state: &RawValue,
    body: &str,
    assets: &mut IndexMap<String, Asset>,
) -> Result<Option<String>, KppError> {
    let origin = Origin::new(DIALECT);
    let mut n = 0usize;
    rewrite(state, &mut |value| {
        if !is_image_part(value) {
            return Ok(None);
        }
        let Ok(part) = Ext::<ImagePartWire>::from_raw(value) else {
            return Ok(None);
        };
        let Ext {
            known: part,
            extra,
            verbatim: _,
        } = part;
        let Ext {
            known: ImageUrlWire { url },
            extra: url_extra,
            verbatim: _,
        } = part.image_url;
        let mut extras = Extras::new(origin.clone()).with_form(STATE_PART, true.into());
        let source = match crate::wire::parse_data_url(&url) {
            Some(data) => {
                if let Some(header) = data.header {
                    extras
                        .form
                        .insert(DATA_URL_HEADER.to_owned(), header.into());
                }
                crate::media::from_base64(data.media_type.to_ascii_lowercase(), data.data, "state")?
            }
            None => Source::Url(url),
        };
        extras.fields.extend(extra);
        for (k, v) in url_extra {
            extras.fields.insert(format!("{IMAGE_PART}.{k}"), v);
        }
        let reference = fresh_reference(body, assets, n);
        n += 1;
        let mut asset = Asset::new(source);
        asset.extras = extras;
        let written = quoted(&reference);
        assets.insert(reference, asset);
        Ok(Some(written))
    })
}

/// An image as an OpenAI-style part, with the fields the app's part carried.
pub(super) fn image_part(asset: &Asset, dry: bool) -> Result<String, KppError> {
    let url = match &asset.source {
        Source::Url(url) => url.clone(),
        Source::Base64(b) if dry => format!("data:{};base64,", b.media_type),
        Source::Base64(b) => match asset.extras.form_str(DIALECT, DATA_URL_HEADER) {
            Some(header) => b.data_url_with(header),
            None => b.data_url(),
        },
        _ => return Err(KppError::unsupported_by("image source", DIALECT, "")),
    };
    let mut url_extra = IndexMap::new();
    let mut part_extra = IndexMap::new();
    if asset.extras.is_from(DIALECT) {
        for (k, v) in asset.extras.body_fields() {
            match k.strip_prefix("image_url.") {
                Some(inner) => url_extra.insert(inner.to_owned(), v.clone()),
                None => part_extra.insert(k.to_owned(), v.clone()),
            };
        }
    }
    let part = ImagePartWire {
        ty: IMAGE_PART.to_owned(),
        image_url: Ext::with(ImageUrlWire { url }, url_extra, IndexMap::new()),
    };
    crate::wire::to_body(&Ext::with(part, part_extra, IndexMap::new())).and_then(|b| {
        String::from_utf8(b)
            .map_err(|_| KppError::provider("Kaeso wrote an image part that is not UTF-8."))
    })
}

/// An image as a data URL, for providers that take a list of them.
pub(super) fn data_url(asset: &Asset, dry: bool) -> Result<String, KppError> {
    match &asset.source {
        Source::Base64(b) if dry => Ok(format!("data:{};base64,", b.media_type)),
        Source::Base64(b) => Ok(b.data_url()),
        _ => Err(KppError::unsupported_by("image URL", DIALECT, "")),
    }
}

/// Refuses an image `profile`'s provider would refuse: a URL it doesn't fetch, a format, size or
/// animation it doesn't take. The size is read from the file's header.
pub(super) fn check_image(
    source: &Source,
    profile: &SystemOneProfile,
    param: &str,
) -> Result<(), KppError> {
    let name = profile.name;
    let refuse = |feature: &str, message: String| {
        Err(KppError::unsupported(feature, message).with_param(param))
    };
    let data = match source {
        Source::Base64(b) => &b.data,
        Source::Url(_) if profile.image_urls => return Ok(()),
        Source::Url(_) => {
            return refuse(
                "image URL",
                format!(
                    "{name} takes images only as bytes, and Kaeso doesn't fetch image URLs; send the image as a data URL or base64."
                ),
            );
        }
        _ => {
            return refuse(
                "image source",
                format!("{name} takes images only as bytes."),
            );
        }
    };
    if let Some(max) = profile.image_bytes
        && data.len() > max
    {
        return refuse(
            "image size",
            format!(
                "{name} takes images of at most {} MiB; this one has {} bytes.",
                max / (1024 * 1024),
                data.len()
            ),
        );
    }
    let checks_format = !profile.image_types.is_empty();
    if !checks_format
        && profile.image_px.is_none()
        && profile.image_pixels.is_none()
        && profile.animated_images
    {
        return Ok(());
    }
    let types = match profile.image_types {
        [] => "PNG, JPEG, WebP or GIF".to_owned(),
        listed => listed.join(", "),
    };
    let Some(info) = image_info(data) else {
        return refuse(
            "image format",
            format!(
                "{name} takes only these images: {types}; Kaeso can't read this one as any of them. Convert it before sending."
            ),
        );
    };
    if checks_format && !profile.image_types.contains(&info.kind.media_type()) {
        return refuse(
            "image format",
            format!(
                "{name} takes only these images: {types}, not {}. Convert it before sending.",
                info.kind.media_type()
            ),
        );
    }
    if info.animated && !profile.animated_images {
        return refuse(
            "animated image",
            format!("{name} takes only still images; send one frame as a PNG or JPEG."),
        );
    }
    if let Some(max) = profile.image_pixels
        && u64::from(info.width) * u64::from(info.height) > max
    {
        return refuse(
            "image size",
            format!(
                "{name} takes images of at most {} megapixels; this one is {}×{}. Resize it before sending.",
                max / 1_000_000,
                info.width,
                info.height
            ),
        );
    }
    if let Some((min, max)) = profile.image_px
        && [info.width, info.height]
            .iter()
            .any(|s| *s < min || *s > max)
    {
        return refuse(
            "image size",
            format!(
                "{name} takes images of {min} to {max} pixels a side and never resizes them; this one is {}×{}. Resize it before sending.",
                info.width, info.height
            ),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewriting_keeps_order_and_bytes_of_what_it_leaves() {
        let text = r#"{"b": [1, "x", {"z": "x"}], "a":"x" ,"c":{"d":1.50}}"#;
        let raw: &RawValue = serde_json::from_str(text).unwrap();
        let out = rewrite(raw, &mut |v| {
            Ok((v.get() == "\"x\"").then(|| "\"y\"".to_owned()))
        })
        .unwrap()
        .unwrap();
        assert_eq!(out, r#"{"b":[1,"y",{"z":"y"}],"a":"y","c":{"d":1.50}}"#);
        let none = rewrite(raw, &mut |_| Ok(None)).unwrap();
        assert_eq!(none, None);
    }

    #[test]
    fn fresh_references_occur_nowhere_else() {
        let taken = IndexMap::new();
        assert_eq!(
            fresh_reference("kaeso-image-0-ref kaeso-image-1-ref", &taken, 0),
            "kaeso-image-2-ref"
        );
        let mut taken = IndexMap::new();
        taken.insert(
            "image-0".to_owned(),
            Asset::new(Source::Url("https://example.com/a.png".into())),
        );
        assert_eq!(fresh_reference("", &taken, 0), "kaeso-image-1-ref");
    }
}
