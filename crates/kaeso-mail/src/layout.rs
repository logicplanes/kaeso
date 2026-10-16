use maud::{DOCTYPE, Markup, PreEscaped, html};

use crate::assets;
use crate::locale::Locale;
use crate::style::{self, class};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    pub name: String,
    pub url: String,
}

impl Instance {
    pub fn new(public_url: &str) -> Instance {
        let name = public_url
            .split_once("://")
            .map_or(public_url, |(_, rest)| rest)
            .trim_end_matches('/')
            .to_owned();
        Instance {
            name,
            url: public_url.to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    Request,
    Security,
    Org,
    Alert,
}

impl Reason {
    fn tail(self, lang: Locale) -> &'static str {
        match (lang, self) {
            (_, Reason::Request) => match lang {
                Locale::ZhCn => "。",
                Locale::En | Locale::De => ".",
            },
            (Locale::En, Reason::Security) => "; security notices are always sent.",
            (Locale::En, Reason::Org) => "; organization notices are always sent.",
            (Locale::En, Reason::Alert) => {
                "; turn alerts off in your account's notification settings."
            }
            (Locale::De, Reason::Security) => "; Sicherheitshinweise werden immer gesendet.",
            (Locale::De, Reason::Org) => "; Hinweise zu Organisationen werden immer gesendet.",
            (Locale::De, Reason::Alert) => {
                "; Warnungen schalten Sie in den Benachrichtigungseinstellungen Ihres Kontos ab."
            }
            (Locale::ZhCn, Reason::Security) => "；安全通知总会发送。",
            (Locale::ZhCn, Reason::Org) => "；组织通知总会发送。",
            (Locale::ZhCn, Reason::Alert) => "；可在账户的通知设置中关闭提醒。",
        }
    }
}

fn footer(lang: Locale) -> (&'static str, &'static str) {
    match lang {
        Locale::En => ("Sent by Kaeso at ", ""),
        Locale::De => ("Gesendet von Kaeso unter ", ""),
        Locale::ZhCn => ("此邮件由 ", " 上的 Kaeso 发送"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Action {
    pub label: &'static str,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Content {
    pub heading: String,
    pub sentence: Option<String>,
    pub action: Option<Action>,
    pub note: Option<String>,
    pub reason: Reason,
    pub lang: Locale,
}

impl Content {
    pub(crate) fn new(lang: Locale, heading: impl Into<String>, reason: Reason) -> Content {
        Content {
            heading: heading.into(),
            sentence: None,
            action: None,
            note: None,
            reason,
            lang,
        }
    }

    pub(crate) fn sentence(mut self, s: impl Into<String>) -> Content {
        self.sentence = Some(s.into());
        self
    }

    pub(crate) fn action(mut self, label: &'static str, url: &str) -> Content {
        self.action = Some(Action {
            label,
            url: url.to_owned(),
        });
        self
    }

    pub(crate) fn note(mut self, s: impl Into<String>) -> Content {
        self.note = Some(s.into());
        self
    }

    fn preview(&self) -> &str {
        self.sentence
            .as_deref()
            .or(self.note.as_deref())
            .unwrap_or(&self.heading)
    }
}

pub(crate) fn text(content: &Content, instance: &Instance) -> String {
    let mut out = format!("{}\n\n", content.heading);
    if let Some(s) = &content.sentence {
        out.push_str(s);
        out.push_str("\n\n");
    }
    if let Some(a) = &content.action {
        let colon = match content.lang {
            Locale::ZhCn => "：",
            Locale::En | Locale::De => ":",
        };
        out.push_str(&format!("{}{colon}\n{}\n\n", a.label, a.url));
    }
    if let Some(n) = &content.note {
        out.push_str(n);
        out.push_str("\n\n");
    }
    let (before, after) = footer(content.lang);
    out.push_str(&format!(
        "-- \n{before}{}{after}{}\n",
        instance.name,
        content.reason.tail(content.lang)
    ));
    out
}

pub(crate) fn html(subject: &str, content: &Content, instance: &Instance) -> String {
    let page: Markup = html! {
        (DOCTYPE)
        (PreEscaped(format!(r#"<html lang="{}" xmlns="http://www.w3.org/1999/xhtml" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office">"#, content.lang.tag())))
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";
            meta name="x-apple-disable-message-reformatting";
            meta name="format-detection" content="telephone=no, date=no, address=no, email=no, url=no";
            meta name="color-scheme" content="light dark";
            meta name="supported-color-schemes" content="light dark";
            title { (subject) }
            (PreEscaped(OUTLOOK_HEAD))
            style { (PreEscaped(style::STYLESHEET)) }
        }
        body.(class::BG) style=(style::CANVAS_STYLE) {
            div style=(style::PREHEADER) aria-hidden="true" {
                (content.preview())
                (PreEscaped(PREVIEW_FILL))
            }
            div.(class::BG) role="article" aria-roledescription="email" aria-label=(subject) lang=(content.lang.tag()) style=(style::CANVAS_STYLE) {
                table.(class::BG) role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" bgcolor=(style::CANVAS) style=(style::CANVAS_STYLE) {
                    tr { td align="left" style="padding:26px 0 48px 26px;" {
                        (PreEscaped(OUTLOOK_COLUMN_OPEN))
                        div style="max-width:562px;" {
                            table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" {
                                tr { td style="padding:0 0 46px 0;" { (logo()) } }
                                (body(content, instance))
                            }
                        }
                        (PreEscaped(OUTLOOK_COLUMN_CLOSE))
                    } }
                }
            }
        }
        (PreEscaped("</html>"))
    };
    page.into_string()
}

fn body(content: &Content, instance: &Instance) -> Markup {
    html! {
        tr { td style="padding:0 28px 0 2px;" {
            h1.(class::TEXT) style=(style::HEADING) { (content.heading) }
        } }
        @if let Some(s) = &content.sentence {
            tr { td style="padding:12px 28px 0 2px;" {
                p.(class::SECONDARY) style=(style::SENTENCE) { (s) }
            } }
        }
        @if let Some(a) = &content.action {
            tr { td style="padding:32px 28px 0 2px;" { (button(a)) } }
        }
        @if let Some(n) = &content.note {
            tr { td style="padding:32px 28px 0 2px;" {
                p.(class::TERTIARY) style=(style::NOTE) { (n) }
            } }
        }
        tr { td style="padding:56px 28px 0 2px;" {
            p.(class::TERTIARY) style=(style::FOOTER) {
                (footer(content.lang).0)
                a href=(instance.url) style=(style::FOOTER_LINK) { (instance.name) }
                (footer(content.lang).1)
                (content.reason.tail(content.lang))
            }
        } }
    }
}

fn logo() -> Markup {
    let light = assets::WORDMARK_LIGHT;
    let dark = assets::WORDMARK_DARK;
    html! {
        img.(class::LOGO_LIGHT) src={ "cid:" (light.cid) } width=(light.width) height=(light.height) alt="Kaeso" style=(style::LOGO);
        (PreEscaped("<!--[if !mso]><!-->"))
        div.(class::LOGO_DARK) style=(style::LOGO_DARK_WRAP) {
            img src={ "cid:" (dark.cid) } width=(dark.width) height=(dark.height) alt="Kaeso" style=(style::LOGO);
        }
        (PreEscaped("<!--<![endif]-->"))
    }
}

fn button(action: &Action) -> Markup {
    let width = vml_width(action.label);
    let href = escape(&action.url);
    let label = escape(action.label);
    let vml = format!(
        r#"<!--[if mso]><v:roundrect xmlns:v="urn:schemas-microsoft-com:vml" xmlns:w="urn:schemas-microsoft-com:office:word" href="{href}" style="height:48px;v-text-anchor:middle;width:{width}px;" arcsize="50%" stroke="f" fillcolor="{ink}"><w:anchorlock/><center style="color:{on_ink};font-family:'Segoe UI',Arial,sans-serif;font-size:15px;font-weight:600;">{label}</center></v:roundrect><![endif]-->"#,
        ink = style::INK,
        on_ink = style::ON_INK,
    );
    html! {
        (PreEscaped(vml))
        (PreEscaped("<!--[if !mso]><!-->"))
        a.(class::BUTTON) href=(action.url) target="_blank" style=(style::BUTTON) { (action.label) }
        (PreEscaped("<!--<![endif]-->"))
    }
}

fn vml_width(label: &str) -> usize {
    label
        .chars()
        .map(|c| if is_wide(c) { 16 } else { 9 })
        .sum::<usize>()
        + 44
}

fn is_wide(c: char) -> bool {
    matches!(c, '\u{2e80}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{ff00}'..='\u{ff60}' | '\u{20000}'..='\u{3134f}')
}

fn escape(s: &str) -> String {
    html! { (s) }.into_string()
}

const OUTLOOK_HEAD: &str = concat!(
    "<!--[if mso]><noscript><xml><o:OfficeDocumentSettings><o:AllowPNG/><o:PixelsPerInch>96</o:PixelsPerInch></o:OfficeDocumentSettings></xml></noscript>",
    "<style>table,td,div,p,a,h1{font-family:'Segoe UI',Arial,sans-serif!important}</style><![endif]-->",
);

const OUTLOOK_COLUMN_OPEN: &str = r#"<!--[if mso]><table role="presentation" width="562" cellpadding="0" cellspacing="0" border="0" align="left"><tr><td><![endif]-->"#;
const OUTLOOK_COLUMN_CLOSE: &str = "<!--[if mso]></td></tr></table><![endif]-->";

const PREVIEW_FILL: &str = "&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instance_is_named_by_its_host() {
        let i = Instance::new("https://kaeso.example.com/");
        assert_eq!(i.name, "kaeso.example.com");
        assert_eq!(
            Instance::new("http://localhost:3000/").name,
            "localhost:3000"
        );
    }

    #[test]
    fn the_vml_button_escapes_what_it_embeds() {
        let b = button(&Action {
            label: "Sign in",
            url: "https://kaeso.example.com/a?x=1&y=\"2\"".to_owned(),
        })
        .into_string();
        assert!(b.contains("href=\"https://kaeso.example.com/a?x=1&amp;y=&quot;2&quot;\""));
        assert!(!b.contains("y=\"2\""));
    }
}
