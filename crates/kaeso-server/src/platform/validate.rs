use super::error::{ApiError, ApiResult};

fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

pub(crate) fn validate_name(param: &str, name: &str) -> ApiResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ApiError::invalid(
            param,
            format!("`{param}` must be 1 to 100 characters."),
        ));
    }
    if name.chars().any(hidden) {
        return Err(ApiError::invalid(
            param,
            format!("`{param}` can't contain line breaks or invisible characters."),
        ));
    }
    Ok(name.to_owned())
}

pub(crate) fn display_name(name: &str) -> Option<String> {
    validate_name("name", name).ok()
}

pub(crate) fn validate_email(param: &str, email: &str) -> ApiResult<String> {
    kaeso_core::email::normalize(email).map_err(|e| ApiError::invalid(param, format!("{e}.")))
}

#[cfg(test)]
mod tests {
    use super::{display_name, validate_name};

    #[test]
    fn names_are_one_visible_line() {
        assert_eq!(
            validate_name("name", "  Ada Lovelace ").unwrap(),
            "Ada Lovelace"
        );
        assert_eq!(validate_name("name", "Bücher GmbH").unwrap(), "Bücher GmbH");
        assert_eq!(validate_name("name", "数据平台").unwrap(), "数据平台");
        for bad in [
            "Acme\nReset your password at https://evil.example",
            "Acme\r\nBcc: x",
            "Acme\u{202E}gnp.exe",
            "Ac\u{200B}me",
            "Acme\u{2066}x",
            "Acme\u{0007}",
            "Acme\u{FEFF}",
        ] {
            let e = validate_name("name", bad).unwrap_err();
            assert_eq!(
                e.to_string(),
                "`name` can't contain line breaks or invisible characters.",
                "{bad:?}"
            );
            assert_eq!(display_name(bad), None);
        }
    }
}
