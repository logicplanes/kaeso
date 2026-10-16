use time::{Date, Month};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Locale {
    #[default]
    En,
    De,
    ZhCn,
}

impl Locale {
    pub const ALL: [Locale; 3] = [Locale::En, Locale::De, Locale::ZhCn];

    pub fn tag(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::De => "de",
            Locale::ZhCn => "zh-CN",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Locale> {
        let primary = tag.trim().split(['-', '_']).next().unwrap_or_default();
        match primary.to_ascii_lowercase().as_str() {
            "en" => Some(Locale::En),
            "de" => Some(Locale::De),
            "zh" => Some(Locale::ZhCn),
            _ => None,
        }
    }

    pub fn from_accept_language(header: &str) -> Option<Locale> {
        let mut ranges: Vec<(u16, Locale)> = header
            .split(',')
            .filter_map(|range| {
                let mut parts = range.split(';');
                let tag = parts.next()?.trim();
                let q = parts
                    .filter_map(|p| p.trim().strip_prefix("q="))
                    .next()
                    .map_or(Some(1000), quality)?;
                (q > 0).then_some(())?;
                Some((q, Locale::from_tag(tag)?))
            })
            .collect();
        ranges.sort_by_key(|r| std::cmp::Reverse(r.0));
        ranges.first().map(|(_, l)| *l)
    }

    pub fn choose(chosen: Option<&str>, accept_language: Option<&str>) -> Locale {
        chosen
            .and_then(Locale::from_tag)
            .or_else(|| accept_language.and_then(Locale::from_accept_language))
            .unwrap_or_default()
    }

    pub fn date(self, date: Date) -> String {
        let (y, d) = (date.year(), date.day());
        match self {
            Locale::En => format!("{y:04}-{:02}-{d:02}", u8::from(date.month())),
            Locale::De => format!("{d}. {} {y}", german_month(date.month())),
            Locale::ZhCn => format!("{y}年{}月{d}日", u8::from(date.month())),
        }
    }

    pub fn number(self, plain: &str) -> String {
        let (group, point) = match self {
            Locale::En => return plain.to_owned(),
            Locale::De => ('.', ','),
            Locale::ZhCn => (',', '.'),
        };
        let (sign, digits) = plain
            .strip_prefix('-')
            .map_or(("", plain), |rest| ("-", rest));
        let (int, frac) = digits
            .split_once('.')
            .map_or((digits, None), |(i, f)| (i, Some(f)));
        if !int.bytes().all(|b| b.is_ascii_digit()) {
            return plain.to_owned();
        }
        let mut out = String::from(sign);
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                out.push(group);
            }
            out.push(c);
        }
        if let Some(f) = frac {
            out.push(point);
            out.push_str(f);
        }
        out
    }

    pub fn percent(self, n: u32) -> String {
        match self {
            Locale::De => format!("{n}\u{a0}%"),
            Locale::En | Locale::ZhCn => format!("{n}%"),
        }
    }
}

fn quality(q: &str) -> Option<u16> {
    let q = q.trim();
    let (int, frac) = q.split_once('.').unwrap_or((q, ""));
    if frac.len() > 3 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole = match int {
        "0" => 0,
        "1" => 1000,
        _ => return None,
    };
    let frac: u16 = format!("{frac:0<3}").parse().ok()?;
    let value = whole + frac;
    (value <= 1000).then_some(value)
}

fn german_month(m: Month) -> &'static str {
    match m {
        Month::January => "Jan.",
        Month::February => "Feb.",
        Month::March => "März",
        Month::April => "Apr.",
        Month::May => "Mai",
        Month::June => "Juni",
        Month::July => "Juli",
        Month::August => "Aug.",
        Month::September => "Sept.",
        Month::October => "Okt.",
        Month::November => "Nov.",
        Month::December => "Dez.",
    }
}

pub(crate) fn zh(parts: &[&str]) -> String {
    let mut out = String::new();
    for part in parts.iter().filter(|p| !p.is_empty()) {
        if let (Some(a), Some(b)) = (out.chars().last(), part.chars().next())
            && ((is_han(a) && is_latin(b)) || (is_latin(a) && is_han(b)))
        {
            out.push(' ');
        }
        out.push_str(part);
    }
    out
}

fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{3134f}')
}

fn is_latin(c: char) -> bool {
    c.is_alphanumeric()
        && !is_han(c)
        && !matches!(c, '\u{3000}'..='\u{30ff}' | '\u{ff00}'..='\u{ffef}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn tags_map_to_languages() {
        assert_eq!(Locale::from_tag("de-AT"), Some(Locale::De));
        assert_eq!(Locale::from_tag("zh-TW"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_tag("zh-Hans-CN"), Some(Locale::ZhCn));
        assert_eq!(Locale::from_tag("EN-gb"), Some(Locale::En));
        assert_eq!(Locale::from_tag("fr"), None);
        assert_eq!(Locale::from_tag(""), None);
        for l in Locale::ALL {
            assert_eq!(Locale::from_tag(l.tag()), Some(l));
        }
    }

    #[test]
    fn accept_language_takes_the_first_supported_language() {
        let pick = Locale::from_accept_language;
        assert_eq!(pick("de-DE,de;q=0.9,en;q=0.8"), Some(Locale::De));
        assert_eq!(
            pick("fr-FR,fr;q=0.9,zh-CN;q=0.8,en;q=0.7"),
            Some(Locale::ZhCn)
        );
        assert_eq!(pick("en-US,en;q=0.9,de;q=0.8"), Some(Locale::En));
        assert_eq!(pick("en;q=0.5, de"), Some(Locale::De));
        assert_eq!(pick("de;q=0, en;q=0.1"), Some(Locale::En));
        assert_eq!(pick("fr, *;q=0.5"), None);
        assert_eq!(pick("de;q=abc, zh"), Some(Locale::ZhCn));
        assert_eq!(pick(""), None);
    }

    #[test]
    fn the_persons_choice_wins_then_the_browser_then_english() {
        assert_eq!(Locale::choose(Some("de"), Some("zh-CN,zh")), Locale::De);
        assert_eq!(Locale::choose(Some("en"), Some("de")), Locale::En);
        assert_eq!(Locale::choose(Some("zh-CN"), None), Locale::ZhCn);
        assert_eq!(Locale::choose(None, Some("de-CH,de;q=0.9")), Locale::De);
        assert_eq!(Locale::choose(None, Some("fr")), Locale::En);
        assert_eq!(Locale::choose(None, None), Locale::En);
    }

    #[test]
    fn dates_follow_the_language() {
        let d = date!(2026 - 10 - 14);
        assert_eq!(Locale::En.date(d), "2026-10-14");
        assert_eq!(Locale::De.date(d), "14. Okt. 2026");
        assert_eq!(Locale::ZhCn.date(d), "2026年10月14日");
        assert_eq!(Locale::De.date(date!(2027 - 03 - 01)), "1. März 2027");
        assert_eq!(Locale::En.date(date!(2027 - 03 - 01)), "2027-03-01");
    }

    #[test]
    fn numbers_follow_the_language() {
        assert_eq!(Locale::En.number("1234567.5"), "1234567.5");
        assert_eq!(Locale::De.number("1234567.5"), "1.234.567,5");
        assert_eq!(Locale::ZhCn.number("1234567.5"), "1,234,567.5");
        assert_eq!(Locale::De.number("50"), "50");
        assert_eq!(Locale::De.number("999"), "999");
        assert_eq!(Locale::De.number("1000"), "1.000");
        assert_eq!(Locale::De.number("0.25"), "0,25");
        assert_eq!(Locale::De.number("-1500"), "-1.500");
        assert_eq!(Locale::De.percent(80), "80\u{a0}%");
        assert_eq!(Locale::ZhCn.percent(80), "80%");
    }

    #[test]
    fn chinese_is_spaced_from_latin_and_digits_at_the_seams() {
        assert_eq!(zh(&["Ada", "邀请您加入", "Acme"]), "Ada 邀请您加入 Acme");
        assert_eq!(zh(&["张三", "邀请您加入", "示例"]), "张三邀请您加入示例");
        assert_eq!(zh(&["API 密钥 ", "ci", "，"]), "API 密钥 ci，");
        assert_eq!(
            zh(&["将于 ", "2026年10月14日", "删除"]),
            "将于 2026年10月14日删除"
        );
        assert_eq!(zh(&["Acme中国", "的"]), "Acme中国的");
    }
}
