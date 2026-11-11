use serde_json::{Value, json};

// The MCP Apps extension (SEP-1865): its identifier, the MIME type of an app's HTML resource,
// the `ui://` scheme and where a tool names its view.
// https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx
pub const EXTENSION: &str = "io.modelcontextprotocol/ui";
pub const MIME: &str = "text/html;profile=mcp-app";
pub const SCHEME: &str = "ui://";
// The flat key is deprecated, but hosts and servers built before the nested one still use it.
const FLAT_KEY: &str = "ui/resourceUri";

pub fn client_capabilities() -> Value {
    json!({ "extensions": { EXTENSION: { "mimeTypes": [MIME] } } })
}

pub fn view_of(meta: &Value) -> Option<&str> {
    meta.pointer("/ui/resourceUri")
        .and_then(Value::as_str)
        .or_else(|| meta.get(FLAT_KEY).and_then(Value::as_str))
        .filter(|uri| uri.starts_with(SCHEME) && uri.len() > SCHEME.len())
}

// A portal serves the views of several servers under one connection, so each one's URIs get
// the server's handle in front, the way its tools get it in their names.
pub fn exposed(handle: &str, uri: &str) -> Option<String> {
    uri.strip_prefix(SCHEME)
        .filter(|rest| !rest.is_empty())
        .map(|rest| format!("{SCHEME}{handle}/{rest}"))
}

pub fn rename(meta: &mut Value, from: &str, to: &str) {
    if let Some(v) = meta.pointer_mut("/ui/resourceUri")
        && v.as_str() == Some(from)
    {
        *v = Value::String(to.to_owned());
    }
    if let Some(v) = meta.get_mut(FLAT_KEY)
        && v.as_str() == Some(from)
    {
        *v = Value::String(to.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_names_its_view_under_either_key() {
        assert_eq!(
            view_of(&json!({ "ui": { "resourceUri": "ui://weather/map" } })),
            Some("ui://weather/map")
        );
        assert_eq!(
            view_of(&json!({ "ui/resourceUri": "ui://weather/map" })),
            Some("ui://weather/map")
        );
        assert_eq!(
            view_of(
                &json!({ "ui": { "resourceUri": "ui://a/new" }, "ui/resourceUri": "ui://a/old" })
            ),
            Some("ui://a/new")
        );
        for none in [
            json!({}),
            json!({ "ui": { "resourceUri": "https://x.example" } }),
            json!({ "ui/resourceUri": "ui://" }),
            json!({ "ui": { "resourceUri": 3 } }),
        ] {
            assert_eq!(view_of(&none), None, "{none}");
        }
    }

    #[test]
    fn exposed_views_carry_the_handle_and_rename_both_keys() {
        assert_eq!(
            exposed("wx", "ui://weather/map").as_deref(),
            Some("ui://wx/weather/map")
        );
        assert_eq!(exposed("wx", "https://weather/map"), None);
        let mut meta = json!({ "ui": { "resourceUri": "ui://weather/map", "visibility": ["model", "app"] }, "ui/resourceUri": "ui://weather/map", "other": 1 });
        rename(&mut meta, "ui://weather/map", "ui://wx/weather/map");
        assert_eq!(
            meta,
            json!({ "ui": { "resourceUri": "ui://wx/weather/map", "visibility": ["model", "app"] }, "ui/resourceUri": "ui://wx/weather/map", "other": 1 })
        );
        assert_eq!(
            client_capabilities()["extensions"][EXTENSION]["mimeTypes"],
            json!([MIME])
        );
    }
}
