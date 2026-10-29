/// Characters a one-line label can't show: control characters, and the format characters
/// that are invisible or reorder text, which can make a name look like another
/// (`Acme\u{202E}gnp.exe` reads as `Acmeexe.png`).
pub fn hidden(c: char) -> bool {
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

#[cfg(test)]
mod tests {
    use super::hidden;

    #[test]
    fn invisible_and_reordering_characters_are_hidden() {
        for c in [
            '\0', '\n', '\u{7}', '\u{202E}', '\u{200B}', '\u{2066}', '\u{FEFF}',
        ] {
            assert!(hidden(c), "{c:?}");
        }
        for c in ['a', ' ', '-', 'ü', '数', '\u{1F600}'] {
            assert!(!hidden(c), "{c:?}");
        }
    }
}
