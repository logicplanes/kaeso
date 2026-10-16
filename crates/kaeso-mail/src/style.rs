macro_rules! token {
    (canvas) => { "#ffffff" };
    (canvas_dark) => { "#000000" };
    (text) => { "#0a0a0a" };
    (text_dark) => { "#fafafa" };
    (secondary) => { "#666666" };
    (secondary_dark) => { "#a3a3a3" };
    (tertiary) => { "#707070" };
    (tertiary_dark) => { "#858585" };
    (ink) => { "#0a0a0a" };
    (ink_dark) => { "#fafafa" };
    (on_ink) => { "#ffffff" };
    (on_ink_dark) => { "#000000" };
    (font) => { "Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif" };
}

pub(crate) const CANVAS: &str = token!(canvas);
pub(crate) const INK: &str = token!(ink);
pub(crate) const ON_INK: &str = token!(on_ink);

pub(crate) const CANVAS_STYLE: &str =
    concat!("margin:0;padding:0;background-color:", token!(canvas), ";");

pub(crate) const HEADING: &str = concat!(
    "margin:0;font-family:",
    token!(font),
    ";font-size:28px;line-height:34px;font-weight:600;letter-spacing:-0.6px;color:",
    token!(text),
    ";mso-line-height-rule:exactly;",
);

pub(crate) const SENTENCE: &str = concat!(
    "margin:0;font-family:",
    token!(font),
    ";font-size:15px;line-height:22px;font-weight:400;color:",
    token!(secondary),
    ";mso-line-height-rule:exactly;",
);

pub(crate) const NOTE: &str = concat!(
    "margin:0;font-family:",
    token!(font),
    ";font-size:13px;line-height:18px;font-weight:400;color:",
    token!(tertiary),
    ";mso-line-height-rule:exactly;",
);

pub(crate) const FOOTER: &str = concat!(
    "margin:0;font-family:",
    token!(font),
    ";font-size:12px;line-height:16px;font-weight:400;color:",
    token!(tertiary),
    ";mso-line-height-rule:exactly;",
);

pub(crate) const FOOTER_LINK: &str = concat!("color:", token!(tertiary), ";text-decoration:none;");

pub(crate) const BUTTON: &str = concat!(
    "display:inline-block;padding:13px 22px;border-radius:9999px;background-color:",
    token!(ink),
    ";color:",
    token!(on_ink),
    ";font-family:",
    token!(font),
    ";font-size:15px;line-height:22px;font-weight:500;text-decoration:none;text-align:center;-webkit-text-size-adjust:none;mso-hide:all;",
);

pub(crate) const LOGO: &str = "display:block;width:64px;height:16px;border:0;text-decoration:none;";

pub(crate) const LOGO_DARK_WRAP: &str = "display:none;overflow:hidden;max-height:0;mso-hide:all;";

pub(crate) const PREHEADER: &str = "display:none;overflow:hidden;max-height:0;max-width:0;mso-hide:all;font-size:1px;line-height:1px;";

pub(crate) mod class {
    pub(crate) const BG: &str = "k-bg";
    pub(crate) const TEXT: &str = "k-text";
    pub(crate) const SECONDARY: &str = "k-secondary";
    pub(crate) const TERTIARY: &str = "k-tertiary";
    pub(crate) const BUTTON: &str = "k-button";
    pub(crate) const LOGO_LIGHT: &str = "k-logo-light";
    pub(crate) const LOGO_DARK: &str = "k-logo-dark";
}

macro_rules! dark_rules {
    ($p:literal) => {
        concat!(
            $p, ".k-bg{background-color:", token!(canvas_dark), "!important}",
            $p, ".k-text{color:", token!(text_dark), "!important}",
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
        for c in [
            class::BG,
            class::TEXT,
            class::SECONDARY,
            class::TERTIARY,
            class::BUTTON,
            class::LOGO_LIGHT,
            class::LOGO_DARK,
        ] {
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
}
