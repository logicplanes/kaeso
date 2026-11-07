macro_rules! token {
    (canvas) => { "#ffffff" };
    (canvas_dark) => { "#000000" };
    (panel) => { "#f5f5f5" };
    (panel_dark) => { "#171717" };
    (rule) => { "#e5e5e5" };
    (rule_dark) => { "#262626" };
    (text) => { "#0a0a0a" };
    (text_dark) => { "#fafafa" };
    (secondary) => { "#5c5c5c" };
    (secondary_dark) => { "#a3a3a3" };
    (tertiary) => { "#6b6b6b" };
    (tertiary_dark) => { "#8f8f8f" };
    (ink) => { "#0a0a0a" };
    (ink_dark) => { "#fafafa" };
    (on_ink) => { "#ffffff" };
    (on_ink_dark) => { "#000000" };
    (font) => { "Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif" };
}

macro_rules! text {
    ($size:literal / $line:literal, $weight:literal, $color:ident) => {
        concat!(
            "font-family:",
            token!(font),
            ";font-size:",
            $size,
            "px;line-height:",
            $line,
            "px;font-weight:",
            $weight,
            ";color:",
            token!($color),
            ";mso-line-height-rule:exactly;",
        )
    };
}

pub(crate) const CANVAS: &str = token!(canvas);
pub(crate) const PANEL: &str = token!(panel);
pub(crate) const INK: &str = token!(ink);
pub(crate) const ON_INK: &str = token!(on_ink);

pub(crate) const COLUMN: u32 = 560;

pub(crate) const CANVAS_STYLE: &str =
    concat!("margin:0;padding:0;background-color:", token!(canvas), ";");

pub(crate) const HEADING: &str = concat!(
    "margin:0;",
    text!(28 / 36, 600, text),
    "letter-spacing:-0.56px;"
);

pub(crate) const LEAD: &str = concat!("margin:0;", text!(16 / 24, 400, secondary));

pub(crate) const SECTION: &str = concat!("margin:0;", text!(14 / 20, 600, text));

pub(crate) const EYEBROW: &str = concat!("margin:0;", text!(13 / 20, 500, tertiary));

pub(crate) const BODY: &str = text!(15 / 24, 400, secondary);

pub(crate) const BULLET: &str = concat!(
    text!(15 / 24, 400, tertiary),
    "width:16px;vertical-align:top;"
);

pub(crate) const RULE: &str = concat!("border-top:1px solid ", token!(rule), ";");

pub(crate) const DETAIL_LABEL: &str = concat!(
    text!(14 / 24, 400, tertiary),
    "padding:8px 16px 8px 0;vertical-align:top;border-top:1px solid ",
    token!(rule),
    ";",
);

pub(crate) const DETAIL_VALUE: &str = concat!(
    text!(15 / 24, 400, text),
    "padding:8px 0;vertical-align:top;border-top:1px solid ",
    token!(rule),
    ";",
);

pub(crate) const STAT_VALUE: &str = concat!(
    "margin:0;",
    text!(24 / 32, 600, text),
    "letter-spacing:-0.48px;font-variant-numeric:tabular-nums;white-space:nowrap;",
);

pub(crate) const STAT_LABEL: &str = concat!("margin:0;", text!(13 / 20, 400, tertiary));

pub(crate) const TABLE_HEAD: &str = concat!(
    text!(13 / 20, 400, tertiary),
    "padding:0 0 8px 0;vertical-align:bottom;",
);

pub(crate) const TABLE_CELL: &str = concat!(
    text!(15 / 24, 400, text),
    "padding:8px 0;vertical-align:top;border-top:1px solid ",
    token!(rule),
    ";",
);

pub(crate) const TABLE_GROUP: &str = concat!(
    text!(14 / 20, 600, text),
    "padding:24px 0 8px 0;vertical-align:bottom;",
);

pub(crate) const TABLE_NUMBER: &str = concat!(
    text!(15 / 24, 400, text),
    "padding:8px 0 8px 16px;vertical-align:top;border-top:1px solid ",
    token!(rule),
    ";text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap;",
);

pub(crate) const LINK: &str = concat!(
    "font-family:",
    token!(font),
    ";font-size:15px;line-height:24px;font-weight:500;color:",
    token!(text),
    ";text-decoration:underline;",
);

pub(crate) const NOTE: &str = concat!("margin:0;", text!(13 / 20, 400, tertiary));

pub(crate) const FOOTER: &str = concat!("margin:0;", text!(12 / 18, 400, tertiary));

pub(crate) const FOOTER_LINK: &str = concat!("color:", token!(tertiary), ";text-decoration:none;");

pub(crate) const FOOTNOTE: &str = concat!("margin:8px 0 0 0;", text!(12 / 18, 400, tertiary));

pub(crate) const FOOTNOTE_LINK: &str =
    concat!("color:", token!(tertiary), ";text-decoration:underline;");

pub(crate) const BUTTON: &str = concat!(
    "display:inline-block;padding:12px 24px;border-radius:9999px;background-color:",
    token!(ink),
    ";color:",
    token!(on_ink),
    ";font-family:",
    token!(font),
    ";font-size:15px;line-height:24px;font-weight:500;text-decoration:none;text-align:center;-webkit-text-size-adjust:none;mso-hide:all;",
);

pub(crate) const LOGO: &str = "display:block;width:64px;height:16px;border:0;text-decoration:none;";

pub(crate) const LOGO_DARK_WRAP: &str = "display:none;overflow:hidden;max-height:0;mso-hide:all;";

pub(crate) const COVER: &str = "display:block;width:100%;max-width:560px;height:auto;border:0;border-radius:12px;text-decoration:none;";

pub(crate) const PANEL_STYLE: &str = concat!(
    "padding:16px 24px;border-radius:12px;background-color:",
    token!(panel),
    ";"
);

pub(crate) const PREHEADER: &str = "display:none;overflow:hidden;max-height:0;max-width:0;mso-hide:all;font-size:1px;line-height:1px;";

pub(crate) mod class {
    pub(crate) const BG: &str = "k-bg";
    pub(crate) const PANEL: &str = "k-panel";
    pub(crate) const RULE: &str = "k-rule";
    pub(crate) const TEXT: &str = "k-text";
    pub(crate) const SECONDARY: &str = "k-secondary";
    pub(crate) const TERTIARY: &str = "k-tertiary";
    pub(crate) const BUTTON: &str = "k-button";
    pub(crate) const LOGO_LIGHT: &str = "k-logo-light";
    pub(crate) const LOGO_DARK: &str = "k-logo-dark";

    #[cfg(test)]
    pub(crate) const ALL: [&str; 9] = [
        BG, PANEL, RULE, TEXT, SECONDARY, TERTIARY, BUTTON, LOGO_LIGHT, LOGO_DARK,
    ];
}

macro_rules! dark_rules {
    ($p:literal) => {
        concat!(
            $p, ".k-bg{background-color:", token!(canvas_dark), "!important}",
            $p, ".k-panel{background-color:", token!(panel_dark), "!important}",
            $p, ".k-rule,", $p, ".k-rule td{border-color:", token!(rule_dark), "!important}",
            $p, ".k-text,", $p, ".k-text a{color:", token!(text_dark), "!important}",
            $p, ".k-secondary{color:", token!(secondary_dark), "!important}",
            $p, ".k-tertiary,", $p, ".k-tertiary a{color:", token!(tertiary_dark), "!important}",
            $p, ".k-button{background-color:", token!(ink_dark), "!important;color:", token!(on_ink_dark), "!important}",
            $p, ".k-logo-light{display:none!important}",
            $p, ".k-logo-dark{display:block!important;max-height:none!important;overflow:visible!important}",
        )
    };
}

pub(crate) const STYLESHEET: &str = concat!(
    ":root{color-scheme:light dark;supported-color-schemes:light dark}",
    "body{margin:0;padding:0;width:100%!important;-webkit-text-size-adjust:100%;-ms-text-size-adjust:100%}",
    "table,td{border-collapse:collapse;mso-table-lspace:0pt;mso-table-rspace:0pt}",
    "img{border:0;outline:none;text-decoration:none;-ms-interpolation-mode:bicubic}",
    "a[x-apple-data-detectors],#MessageViewBody a{color:inherit!important;text-decoration:none!important;font:inherit!important}",
    "@media (max-width:480px){.k-gutter{padding:24px 16px 40px 16px!important}.k-h1{font-size:24px!important;line-height:32px!important}.k-stat-value{font-size:20px!important;line-height:28px!important}.k-dt{width:112px!important}}",
    "@media (prefers-color-scheme:dark){",
    dark_rules!(""),
    "}",
    dark_rules!("[data-ogsc] "),
    dark_rules!("[data-ogsb] "),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dark_rule_is_important() {
        let dark = STYLESHEET
            .split_once("@media (prefers-color-scheme:dark){")
            .map(|(_, rest)| rest)
            .unwrap_or_default();
        for rule in dark.split('}').filter(|r| r.contains('{')) {
            let (_, body) = rule.split_once('{').unwrap_or_default();
            for decl in body.split(';').filter(|d| !d.is_empty()) {
                assert!(decl.ends_with("!important"), "{rule}");
            }
        }
    }

    #[test]
    fn each_class_has_a_dark_rule_for_each_client() {
        let (_, dark) = STYLESHEET
            .split_once("@media (prefers-color-scheme:dark){")
            .unwrap_or_default();
        for c in class::ALL {
            assert!(
                dark.contains(&format!(".{c}{{")) || dark.contains(&format!(".{c},")),
                "{c}"
            );
            for prefix in ["[data-ogsc] ", "[data-ogsb] "] {
                assert!(
                    STYLESHEET.contains(&format!("{prefix}.{c}")),
                    "{prefix}.{c}"
                );
            }
        }
    }

    #[test]
    fn spacing_steps_are_multiples_of_4px() {
        let styles = [
            HEADING,
            LEAD,
            SECTION,
            BODY,
            DETAIL_LABEL,
            DETAIL_VALUE,
            STAT_VALUE,
            TABLE_HEAD,
            TABLE_CELL,
            TABLE_NUMBER,
            BUTTON,
            PANEL_STYLE,
            FOOTNOTE,
        ];
        for style in styles {
            for decl in style.split(';') {
                let Some((prop, value)) = decl.split_once(':') else {
                    continue;
                };
                if !(prop.starts_with("padding") || prop.starts_with("margin")) {
                    continue;
                }
                for px in value.split_whitespace() {
                    let n: u32 = px.trim_end_matches("px").parse().unwrap();
                    assert!(n.is_multiple_of(4), "{style}: {decl}");
                }
            }
        }
    }

    #[test]
    fn text_meets_contrast_on_both_canvases() {
        fn luminance(hex: &str) -> f64 {
            let channel = |i: usize| {
                let c = f64::from(u8::from_str_radix(&hex[i..i + 2], 16).unwrap()) / 255.0;
                if c <= 0.03928 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5)
        }
        fn ratio(a: &str, b: &str) -> f64 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for (fg, bg) in [
            (token!(secondary), token!(canvas)),
            (token!(tertiary), token!(canvas)),
            (token!(tertiary), token!(panel)),
            (token!(secondary_dark), token!(canvas_dark)),
            (token!(tertiary_dark), token!(canvas_dark)),
            (token!(tertiary_dark), token!(panel_dark)),
            (token!(on_ink), token!(ink)),
            (token!(on_ink_dark), token!(ink_dark)),
        ] {
            assert!(ratio(fg, bg) >= 4.5, "{fg} on {bg}: {:.2}", ratio(fg, bg));
        }
    }
}
