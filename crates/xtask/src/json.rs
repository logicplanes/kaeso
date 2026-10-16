use serde_json::value::RawValue;

pub(crate) fn pretty(input: &[u8]) -> Result<String, serde_json::Error> {
    let raw: Box<RawValue> = serde_json::from_slice(input)?;
    let mut out = String::with_capacity(raw.get().len() * 2);
    let mut depth = 0usize;
    let mut chars = raw.get().chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push('"');
                while let Some(s) = chars.next() {
                    out.push(s);
                    match s {
                        '\\' => out.extend(chars.next()),
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '{' | '[' => {
                while chars.next_if(char::is_ascii_whitespace).is_some() {}
                let close = if c == '{' { '}' } else { ']' };
                out.push(c);
                if chars.next_if_eq(&close).is_some() {
                    out.push(close);
                } else {
                    depth += 1;
                    newline(&mut out, depth);
                }
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                newline(&mut out, depth);
                out.push(c);
            }
            ',' => {
                out.push(',');
                newline(&mut out, depth);
            }
            ':' => out.push_str(": "),
            c if c.is_ascii_whitespace() => {}
            c => out.push(c),
        }
    }
    out.push('\n');
    Ok(out)
}

fn newline(out: &mut String, depth: usize) {
    out.push('\n');
    out.extend(std::iter::repeat_n("  ", depth));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indents_by_two_and_keeps_key_order_and_literals() {
        let input =
            br#" {"b":1.50,"a":[1, 2 ,{ }],"s":"x\"y, {z}: [w]","e":[ ],"n":null,"o":{"k":true}} "#;
        let expected = "{\n  \"b\": 1.50,\n  \"a\": [\n    1,\n    2,\n    {}\n  ],\n  \"s\": \"x\\\"y, {z}: [w]\",\n  \"e\": [],\n  \"n\": null,\n  \"o\": {\n    \"k\": true\n  }\n}\n";
        assert_eq!(pretty(input).unwrap(), expected);
    }

    #[test]
    fn keeps_escapes_and_unicode() {
        assert_eq!(
            pretty(r#"["\\", "\u00e9", "é"]"#.as_bytes()).unwrap(),
            "[\n  \"\\\\\",\n  \"\\u00e9\",\n  \"é\"\n]\n"
        );
        assert_eq!(pretty(b"42").unwrap(), "42\n");
    }

    #[test]
    fn refuses_invalid_json() {
        assert!(pretty(b"{\"a\":").is_err());
        assert!(pretty(b"<html>").is_err());
    }
}
