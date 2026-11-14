pub const MAX_WRITTEN: usize = 90;

pub const MAX_SENT: usize = 120;

pub fn problem(text: &str, max: usize) -> Option<String> {
    let len = text.chars().count();
    if len > max {
        return Some(format!("{len} characters, more than {max}"));
    }
    if text.contains('\n') {
        return Some("more than one line".to_owned());
    }
    let chars: Vec<char> = text.chars().collect();
    let mut in_code = false;
    for (i, c) in chars.iter().enumerate() {
        match c {
            '`' => in_code = !in_code,
            '.' | '!' | '?' if !in_code => {
                let rest = chars.get(i + 1..).unwrap_or_default();
                let gap = rest.iter().take_while(|c| c.is_whitespace()).count();
                if gap > 0
                    && rest
                        .get(gap)
                        .is_some_and(|n| n.is_uppercase() || matches!(n, '`' | '{' | '('))
                {
                    return Some("more than one sentence".to_owned());
                }
            }
            _ => {}
        }
    }
    None
}

pub fn sent_problem(message: &str) -> Option<String> {
    problem(message, MAX_SENT)
        .or_else(|| (!message.ends_with('.')).then(|| "does not end with a period".to_owned()))
}
