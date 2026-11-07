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
    Notice,
}

impl Reason {
    fn tail(self, lang: Locale) -> &'static str {
        match (lang, self) {
            (_, Reason::Request | Reason::Notice) => match lang {
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
            (Locale::ZhCn, Reason::Security) => "；安全通知始终会发送。",
            (Locale::ZhCn, Reason::Org) => "；组织通知始终会发送。",
            (Locale::ZhCn, Reason::Alert) => "；可在账户的通知设置中关闭提醒。",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Label {
    Organization,
    Project,
    AppliesTo,
    Budget,
    Used,
    Resets,
    Provider,
    Key,
    Account,
    UsedBy,
    Answer,
    FirstRefused,
    LastRefused,
    LastUsed,
    SignedOut,
    Model,
    InputPrice,
    OutputPrice,
    CacheReadPrice,
    CacheWritePrice,
    CacheWriteHourPrice,
    Version,
    Running,
    Installs,
    Expires,
    TopModels,
}

impl Label {
    pub(crate) fn text(self, lang: Locale) -> &'static str {
        match (lang, self) {
            (Locale::En, Label::Organization) => "Organization",
            (Locale::En, Label::Project) => "Project",
            (Locale::En, Label::AppliesTo) => "Applies to",
            (Locale::En, Label::Budget) => "Budget",
            (Locale::En, Label::Used) => "Used",
            (Locale::En, Label::Resets) => "Resets",
            (Locale::En, Label::Provider) => "Provider",
            (Locale::En, Label::Key) => "Key",
            (Locale::En, Label::Account) => "Account",
            (Locale::En, Label::UsedBy) => "Used by",
            (Locale::En, Label::Answer) => "Provider's answer",
            (Locale::En, Label::FirstRefused) => "First refused",
            (Locale::En, Label::LastRefused) => "Last refused",
            (Locale::En, Label::LastUsed) => "Last used",
            (Locale::En, Label::SignedOut) => "Signed out",
            (Locale::En, Label::Model) => "Model",
            (Locale::En, Label::InputPrice) => "Input price",
            (Locale::En, Label::OutputPrice) => "Output price",
            (Locale::En, Label::CacheReadPrice) => "Cache read price",
            (Locale::En, Label::CacheWritePrice) => "Cache write price",
            (Locale::En, Label::CacheWriteHourPrice) => "Cache write price (1 hour)",
            (Locale::En, Label::Version) => "Version",
            (Locale::En, Label::Running) => "Running now",
            (Locale::En, Label::Installs) => "Installs",
            (Locale::En, Label::Expires) => "Expires",
            (Locale::En, Label::TopModels) => "Top models",
            (Locale::De, Label::Organization) => "Organisation",
            (Locale::De, Label::Project) => "Projekt",
            (Locale::De, Label::AppliesTo) => "Gilt für",
            (Locale::De, Label::Budget) => "Budget",
            (Locale::De, Label::Used) => "Verbraucht",
            (Locale::De, Label::Resets) => "Zurückgesetzt",
            (Locale::De, Label::Provider) => "Anbieter",
            (Locale::De, Label::Key) => "Schlüssel",
            (Locale::De, Label::Account) => "Konto",
            (Locale::De, Label::UsedBy) => "Verwendet von",
            (Locale::De, Label::Answer) => "Antwort des Anbieters",
            (Locale::De, Label::FirstRefused) => "Erste Ablehnung",
            (Locale::De, Label::LastRefused) => "Letzte Ablehnung",
            (Locale::De, Label::LastUsed) => "Zuletzt verwendet",
            (Locale::De, Label::SignedOut) => "Abgemeldet",
            (Locale::De, Label::Model) => "Modell",
            (Locale::De, Label::InputPrice) => "Eingabepreis",
            (Locale::De, Label::OutputPrice) => "Ausgabepreis",
            (Locale::De, Label::CacheReadPrice) => "Preis für Cache-Lesen",
            (Locale::De, Label::CacheWritePrice) => "Preis für Cache-Schreiben",
            (Locale::De, Label::CacheWriteHourPrice) => "Preis für Cache-Schreiben (1 Stunde)",
            (Locale::De, Label::Version) => "Version",
            (Locale::De, Label::Running) => "Läuft jetzt",
            (Locale::De, Label::Installs) => "Installation",
            (Locale::De, Label::Expires) => "Läuft ab",
            (Locale::De, Label::TopModels) => "Meistgenutzte Modelle",
            (Locale::ZhCn, Label::Organization) => "组织",
            (Locale::ZhCn, Label::Project) => "项目",
            (Locale::ZhCn, Label::AppliesTo) => "适用于",
            (Locale::ZhCn, Label::Budget) => "预算",
            (Locale::ZhCn, Label::Used) => "已用",
            (Locale::ZhCn, Label::Resets) => "重置时间",
            (Locale::ZhCn, Label::Provider) => "服务商",
            (Locale::ZhCn, Label::Key) => "密钥",
            (Locale::ZhCn, Label::Account) => "账户",
            (Locale::ZhCn, Label::UsedBy) => "使用方",
            (Locale::ZhCn, Label::Answer) => "服务商的回复",
            (Locale::ZhCn, Label::FirstRefused) => "首次拒绝",
            (Locale::ZhCn, Label::LastRefused) => "最近拒绝",
            (Locale::ZhCn, Label::LastUsed) => "最近使用",
            (Locale::ZhCn, Label::SignedOut) => "退出登录时间",
            (Locale::ZhCn, Label::Model) => "模型",
            (Locale::ZhCn, Label::InputPrice) => "输入价格",
            (Locale::ZhCn, Label::OutputPrice) => "输出价格",
            (Locale::ZhCn, Label::CacheReadPrice) => "缓存读取价格",
            (Locale::ZhCn, Label::CacheWritePrice) => "缓存写入价格",
            (Locale::ZhCn, Label::CacheWriteHourPrice) => "缓存写入价格（1 小时）",
            (Locale::ZhCn, Label::Version) => "版本",
            (Locale::ZhCn, Label::Running) => "当前版本",
            (Locale::ZhCn, Label::Installs) => "安装方式",
            (Locale::ZhCn, Label::Expires) => "过期时间",
            (Locale::ZhCn, Label::TopModels) => "常用模型",
        }
    }

    pub(crate) fn of(self, lang: Locale, value: impl Into<String>) -> Detail {
        Detail {
            label: self.text(lang),
            value: value.into(),
        }
    }
}

pub(crate) fn never(lang: Locale) -> &'static str {
    match lang {
        Locale::En => "never",
        Locale::De => "nie",
        Locale::ZhCn => "从不",
    }
}

fn footer(lang: Locale) -> (&'static str, &'static str) {
    match lang {
        Locale::En => ("Sent by Kaeso at ", ""),
        Locale::De => ("Gesendet von Kaeso unter ", ""),
        Locale::ZhCn => ("此邮件由 ", " 上的 Kaeso 发送"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cover {
    Release,
    Welcome,
    Summary,
    Costs,
    Apps,
}

impl Cover {
    pub(crate) fn image(self) -> assets::Image {
        match self {
            Cover::Release => assets::COVER_RELEASE,
            Cover::Welcome => assets::COVER_WELCOME,
            Cover::Summary => assets::COVER_SUMMARY,
            Cover::Costs => assets::COVER_COSTS,
            Cover::Apps => assets::COVER_APPS,
        }
    }

    pub(crate) fn alt(self, lang: Locale) -> &'static str {
        match (lang, self) {
            (Locale::En, Cover::Release) => {
                "The Kaeso wordmark beside release art of orange, yellow and blue shapes"
            }
            (Locale::En, Cover::Welcome) => {
                "The Kaeso wordmark beside arcs of teal, coral and gold"
            }
            (Locale::En, Cover::Summary) => {
                "The Kaeso wordmark beside rows of peaks like a usage chart"
            }
            (Locale::En, Cover::Costs) => "The Kaeso wordmark beside nested orange and red squares",
            (Locale::En, Cover::Apps) => "The Kaeso wordmark beside a pattern of linked red curves",
            (Locale::De, Cover::Release) => {
                "Der Kaeso-Schriftzug neben Release-Grafik aus orangen, gelben und blauen Formen"
            }
            (Locale::De, Cover::Welcome) => {
                "Der Kaeso-Schriftzug neben Bögen in Petrol, Koralle und Gold"
            }
            (Locale::De, Cover::Summary) => {
                "Der Kaeso-Schriftzug neben Reihen von Spitzen wie in einem Nutzungsdiagramm"
            }
            (Locale::De, Cover::Costs) => {
                "Der Kaeso-Schriftzug neben ineinanderliegenden orangen und roten Quadraten"
            }
            (Locale::De, Cover::Apps) => {
                "Der Kaeso-Schriftzug neben einem Muster aus verbundenen roten Bögen"
            }
            (Locale::ZhCn, Cover::Release) => "Kaeso 字标，旁边是由橙、黄、蓝色图形组成的版本插图",
            (Locale::ZhCn, Cover::Welcome) => "Kaeso 字标，旁边是青绿、珊瑚红和金色的弧线",
            (Locale::ZhCn, Cover::Summary) => "Kaeso 字标，旁边是一排排如用量图表般的峰线",
            (Locale::ZhCn, Cover::Costs) => "Kaeso 字标，旁边是层层嵌套的橙色和红色方块",
            (Locale::ZhCn, Cover::Apps) => "Kaeso 字标，旁边是由相连红色弧线组成的图案",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Action {
    pub label: &'static str,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Link {
    pub label: &'static str,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Detail {
    pub label: &'static str,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stat {
    pub label: &'static str,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Row {
    pub cells: Vec<String>,
    pub text: String,
    pub heading: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Table {
    pub head: Vec<&'static str>,
    pub rows: Vec<Row>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct List {
    pub label: Option<&'static str>,
    pub items: Vec<String>,
    pub groups: Vec<Group>,
    pub more: Option<String>,
    pub lang: Option<Locale>,
}

impl List {
    fn is_empty(&self) -> bool {
        self.items.is_empty() && self.groups.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Group {
    pub title: String,
    pub items: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Footnote {
    pub because: String,
    pub links: Vec<Link>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Content {
    pub heading: String,
    pub eyebrow: Option<String>,
    pub sentence: Option<String>,
    pub preheader: Option<String>,
    pub cover: Option<Cover>,
    pub stats: Vec<Stat>,
    pub details: Vec<Detail>,
    pub table: Option<Table>,
    pub list: List,
    pub action: Option<Action>,
    pub link: Option<Link>,
    pub note: Option<String>,
    pub footnote: Option<Footnote>,
    pub reason: Reason,
    pub lang: Locale,
}

impl Content {
    pub(crate) fn new(lang: Locale, heading: impl Into<String>, reason: Reason) -> Content {
        Content {
            heading: heading.into(),
            eyebrow: None,
            sentence: None,
            preheader: None,
            cover: None,
            stats: Vec::new(),
            details: Vec::new(),
            table: None,
            list: List::default(),
            action: None,
            link: None,
            note: None,
            footnote: None,
            reason,
            lang,
        }
    }

    pub(crate) fn cover(mut self, cover: Cover) -> Content {
        self.cover = Some(cover);
        self
    }

    pub(crate) fn stats(mut self, stats: Vec<Stat>) -> Content {
        self.stats = stats;
        self
    }

    pub(crate) fn details(mut self, details: Vec<Detail>) -> Content {
        self.details = details;
        self
    }

    pub(crate) fn table(mut self, table: Option<Table>) -> Content {
        self.table = table;
        self
    }

    pub(crate) fn list(mut self, list: List) -> Content {
        self.list = list;
        self
    }

    pub(crate) fn footnote(mut self, footnote: Footnote) -> Content {
        self.footnote = Some(footnote);
        self
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

    pub(crate) fn link(mut self, link: Option<Link>) -> Content {
        self.link = link;
        self
    }

    pub(crate) fn eyebrow(mut self, eyebrow: Option<String>) -> Content {
        self.eyebrow = eyebrow;
        self
    }

    pub(crate) fn preheader(mut self, s: impl Into<String>) -> Content {
        self.preheader = Some(s.into());
        self
    }

    pub(crate) fn note(mut self, s: impl Into<String>) -> Content {
        self.note = Some(s.into());
        self
    }

    pub(crate) fn images(&self) -> Vec<assets::Image> {
        match self.cover {
            Some(cover) => vec![cover.image()],
            None => assets::WORDMARKS.to_vec(),
        }
    }

    fn preview(&self) -> &str {
        self.preheader
            .as_deref()
            .or(self.sentence.as_deref())
            .or(self.note.as_deref())
            .unwrap_or(&self.heading)
    }
}

pub(crate) fn text(content: &Content, instance: &Instance) -> String {
    let mut out = format!("{}\n\n", content.heading);
    if let Some(e) = &content.eyebrow {
        out.push_str(e);
        out.push_str("\n\n");
    }
    if let Some(s) = &content.sentence {
        out.push_str(s);
        out.push_str("\n\n");
    }
    let colon = match content.lang {
        Locale::ZhCn => "：",
        Locale::En | Locale::De => ":",
    };
    let mut block = |lines: Vec<String>| {
        if !lines.is_empty() {
            out.push_str(&lines.join("\n"));
            out.push_str("\n\n");
        }
    };
    block(
        content
            .stats
            .iter()
            .map(|s| format!("{}{colon} {}", s.label, s.value))
            .collect(),
    );
    block(
        content
            .details
            .iter()
            .map(|d| format!("{}{colon} {}", d.label, d.value))
            .collect(),
    );
    if let Some(t) = &content.table {
        block(
            t.rows
                .iter()
                .map(|r| {
                    if r.heading {
                        r.text.clone()
                    } else {
                        format!("- {}", r.text)
                    }
                })
                .collect(),
        );
    }
    let list = &content.list;
    if !list.is_empty() {
        block(
            list.label
                .map(str::to_owned)
                .into_iter()
                .chain(list.items.iter().map(|i| format!("- {i}")))
                .chain(list.groups.iter().flat_map(|g| {
                    std::iter::once(g.title.clone()).chain(g.items.iter().map(|i| format!("- {i}")))
                }))
                .chain(list.more.clone())
                .collect(),
        );
    }
    if let Some(a) = &content.action {
        block(vec![format!("{}{colon}", a.label), a.url.clone()]);
    }
    if let Some(l) = &content.link {
        block(vec![format!("{}{colon}", l.label), l.url.clone()]);
    }
    if let Some(n) = &content.note {
        block(vec![n.clone()]);
    }
    let (before, after) = footer(content.lang);
    out.push_str(&format!(
        "-- \n{before}{}{after}{}\n",
        instance.name,
        content.reason.tail(content.lang)
    ));
    if let Some(f) = &content.footnote {
        out.push_str(&f.because);
        out.push('\n');
        for link in &f.links {
            out.push_str(&format!("{}{colon} {}\n", link.label, link.url));
        }
    }
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
                    tr { td.k-gutter align="left" style="padding:40px 32px 48px 32px;" {
                        (PreEscaped(OUTLOOK_COLUMN_OPEN))
                        div style={ "max-width:" (style::COLUMN) "px;" } {
                            table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" {
                                @match content.cover {
                                    Some(c) => tr { td style="padding:0 0 32px 0;" { (cover(c, content.lang)) } },
                                    None => tr { td style="padding:0 0 40px 0;" { (logo()) } },
                                }
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
    let list = &content.list;
    html! {
        @if let Some(e) = &content.eyebrow {
            tr { td style="padding:0 0 8px 0;" {
                p.(class::TERTIARY) style=(style::EYEBROW) { (e) }
            } }
        }
        tr { td {
            h1.(class::TEXT).k-h1 style=(style::HEADING) { (content.heading) }
        } }
        @if let Some(s) = &content.sentence {
            tr { td style="padding:12px 0 0 0;" {
                p.(class::SECONDARY) style=(style::LEAD) { (s) }
            } }
        }
        @if !content.stats.is_empty() {
            tr { td style="padding:32px 0 0 0;" { (stats(&content.stats)) } }
        }
        @if !content.details.is_empty() {
            tr { td style="padding:24px 0 0 0;" { (details(&content.details)) } }
        }
        @if let Some(t) = &content.table {
            tr { td style="padding:32px 0 0 0;" { (table(t)) } }
        }
        @if !list.is_empty() {
            tr { td style="padding:32px 0 0 0;" { (items(list)) } }
        }
        @if let Some(a) = &content.action {
            tr { td style="padding:32px 0 0 0;" { (button(a)) } }
        }
        @if let Some(l) = &content.link {
            tr { td style="padding:16px 0 0 0;" {
                a.(class::TEXT) href=(l.url) target="_blank" style=(style::LINK) { (l.label) }
            } }
        }
        @if let Some(n) = &content.note {
            tr { td style="padding:24px 0 0 0;" {
                p.(class::TERTIARY) style=(style::NOTE) { (n) }
            } }
        }
        tr { td style="padding:48px 0 0 0;" {
            table.(class::RULE) role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" {
                tr { td style={ (style::RULE) "padding:24px 0 0 0;" } {
                    p.(class::TERTIARY) style=(style::FOOTER) {
                        (footer(content.lang).0)
                        a href=(instance.url) style=(style::FOOTER_LINK) { (instance.name) }
                        (footer(content.lang).1)
                        (content.reason.tail(content.lang))
                    }
                    @if let Some(f) = &content.footnote {
                        p.(class::TERTIARY) style=(style::FOOTNOTE) {
                            (f.because)
                            @for (i, link) in f.links.iter().enumerate() {
                                @if i == 0 { " " } @else { " · " }
                                a href=(link.url) style=(style::FOOTNOTE_LINK) { (link.label) }
                            }
                        }
                    }
                } }
            }
        } }
    }
}

fn cover(cover: Cover, lang: Locale) -> Markup {
    let image = cover.image();
    html! {
        img src={ "cid:" (image.cid) } width=(image.width) height=(image.height) alt=(cover.alt(lang)) style=(style::COVER);
    }
}

fn stats(stats: &[Stat]) -> Markup {
    let last = stats.len().saturating_sub(1);
    let width = format!("{}%", 100 / stats.len().max(1));
    html! {
        table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" {
            tr { td.(class::PANEL) bgcolor=(style::PANEL) style=(style::PANEL_STYLE) {
                table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" {
                    tr {
                        @for (i, s) in stats.iter().enumerate() {
                            td valign="top" width=(width) style={ "padding:0 " (if i == last { "0" } else { "16px" }) " 0 0;" } {
                                p.(class::TEXT).k-stat-value style=(style::STAT_VALUE) { (s.value) }
                                p.(class::TERTIARY) style=(style::STAT_LABEL) { (s.label) }
                            }
                        }
                    }
                }
            } }
        }
    }
}

fn details(details: &[Detail]) -> Markup {
    html! {
        table.(class::RULE) role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" {
            @for d in details {
                tr {
                    td.(class::TERTIARY).k-dt width="168" style=(style::DETAIL_LABEL) { (d.label) }
                    td.(class::TEXT) style=(style::DETAIL_VALUE) { (d.value) }
                }
            }
            tr { td colspan="2" style={ (style::RULE) "font-size:0;line-height:0;" } { "\u{200b}" } }
        }
    }
}

fn table(t: &Table) -> Markup {
    let columns = t.head.len();
    html! {
        table.(class::RULE) role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" {
            tr {
                @for (i, h) in t.head.iter().enumerate() {
                    @if i == 0 {
                        td.(class::TERTIARY) align="left" style=(style::TABLE_HEAD) { (h) }
                    } @else {
                        td.(class::TERTIARY) align="right" style={ (style::TABLE_HEAD) "padding-left:16px;text-align:right;" } { (h) }
                    }
                }
            }
            @for (n, row) in t.rows.iter().enumerate() {
                @if row.heading {
                    tr { td.(class::TEXT) colspan=(columns) style={ (style::TABLE_GROUP) (if n == 0 { "padding-top:8px;" } else { "" }) } { (row.cells.concat()) } }
                } @else { tr {
                    @for (i, cell) in row.cells.iter().enumerate() {
                        @if i == 0 {
                            td.(class::TEXT) style=(style::TABLE_CELL) { (cell) }
                        } @else {
                            td.(class::TEXT) align="right" style=(style::TABLE_NUMBER) { (cell) }
                        }
                    }
                } }
            }
            tr { td colspan=(columns) style={ (style::RULE) "font-size:0;line-height:0;" } { "\u{200b}" } }
        }
    }
}

fn items(list: &List) -> Markup {
    html! {
        @if let Some(label) = list.label {
            h2.(class::TEXT) style={ (style::SECTION) "padding:0 0 8px 0;" } { (label) }
        }
        @if !list.items.is_empty() {
            (bullets(&list.items, list.lang))
        }
        @for (i, group) in list.groups.iter().enumerate() {
            p.(class::TERTIARY) style={ (style::EYEBROW) "padding:" (if i == 0 && list.items.is_empty() { "0" } else { "16px" }) " 0 8px 0;" } { (group.title) }
            (bullets(&group.items, list.lang))
        }
        @if let Some(more) = &list.more {
            p.(class::TERTIARY) style={ (style::NOTE) "padding:0 0 0 16px;" } { (more) }
        }
    }
}

fn bullets(items: &[String], lang: Option<Locale>) -> Markup {
    html! {
        table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0" lang=[lang.map(Locale::tag)] {
            @for item in items {
                tr {
                    td.(class::TERTIARY) style={ (style::BULLET) "padding:0 0 8px 0;" } { "•" }
                    td.(class::SECONDARY) style={ (style::BODY) "padding:0 0 8px 0;" } { (item) }
                }
            }
        }
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
        + 48
}

fn is_wide(c: char) -> bool {
    matches!(c, '\u{2e80}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{ff00}'..='\u{ff60}' | '\u{20000}'..='\u{3134f}')
}

fn escape(s: &str) -> String {
    html! { (s) }.into_string()
}

const OUTLOOK_HEAD: &str = concat!(
    "<!--[if mso]><noscript><xml><o:OfficeDocumentSettings><o:AllowPNG/><o:PixelsPerInch>96</o:PixelsPerInch></o:OfficeDocumentSettings></xml></noscript>",
    "<style>table,td,div,p,a,h1,h2{font-family:'Segoe UI',Arial,sans-serif!important}</style><![endif]-->",
);

const OUTLOOK_COLUMN_OPEN: &str = r#"<!--[if mso]><table role="presentation" width="560" cellpadding="0" cellspacing="0" border="0" align="left"><tr><td><![endif]-->"#;
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

    fn page(content: &Content) -> String {
        html(
            "Subject",
            content,
            &Instance::new("https://kaeso.example.com/"),
        )
    }

    #[test]
    fn a_cover_replaces_the_wordmark_and_says_what_it_shows() {
        for lang in Locale::ALL {
            for cover in [
                Cover::Release,
                Cover::Welcome,
                Cover::Summary,
                Cover::Costs,
                Cover::Apps,
            ] {
                let content = Content::new(lang, "Heading", Reason::Notice).cover(cover);
                let html = page(&content);
                let image = cover.image();
                assert!(html.contains(&format!(
                    r#"<img src="cid:{}" width="560" height="240" alt="{}""#,
                    image.cid,
                    cover.alt(lang)
                )));
                assert!(!html.contains("cid:wordmark-light@kaeso"));
                assert!(cover.alt(lang).contains("Kaeso"));
                assert_eq!(content.images(), vec![image]);
                assert!(html.find(image.cid) < html.find("<h1"));
            }
        }
    }

    #[test]
    fn without_a_cover_the_wordmark_heads_the_email_and_no_cover_is_attached() {
        let content = Content::new(Locale::En, "Heading", Reason::Security);
        let html = page(&content);
        assert!(html.contains(r#"src="cid:wordmark-light@kaeso""#));
        assert!(!html.contains("cid:cover-"));
        assert_eq!(content.images(), assets::WORDMARKS.to_vec());
    }

    #[test]
    fn stats_tables_and_lists_render_in_html_and_text() {
        let content = Content::new(Locale::En, "Heading", Reason::Notice)
            .stats(vec![Stat {
                label: "Requests",
                value: "12,480".to_owned(),
            }])
            .table(Some(Table {
                head: vec!["Project", "Cost"],
                rows: vec![
                    Row {
                        cells: vec!["Acme".to_owned()],
                        text: "Acme".to_owned(),
                        heading: true,
                    },
                    Row {
                        cells: vec!["Checkout".to_owned(), "$30.10".to_owned()],
                        text: "Checkout: $30.10".to_owned(),
                        heading: false,
                    },
                ],
            }))
            .list(List {
                label: Some("What changed"),
                items: vec!["Faster streams".to_owned()],
                groups: vec![Group {
                    title: "Acme / Checkout".to_owned(),
                    items: vec!["Content logging is off".to_owned()],
                }],
                more: Some("And 3 more changes.".to_owned()),
                lang: Some(Locale::En),
            })
            .link(Some(Link {
                label: "Read the release notes",
                url: "https://example.com/notes".to_owned(),
            }));
        let html = page(&content);
        for needle in [
            ">12,480</p>",
            ">Requests</p>",
            ">Project</td>",
            r#"align="right" style="#,
            ">$30.10</td>",
            r#"colspan="2" style="#,
            ">Acme</td>",
            ">Acme / Checkout</p>",
            ">Content logging is off</td>",
            ">What changed</h2>",
            r#"lang="en""#,
            ">Faster streams</td>",
            ">And 3 more changes.</p>",
            r#"href="https://example.com/notes""#,
        ] {
            assert!(html.contains(needle), "{needle}");
        }
        let text = text(&content, &Instance::new("https://kaeso.example.com/"));
        assert!(text.contains("Requests: 12,480\n\nAcme\n- Checkout: $30.10\n\nWhat changed\n- Faster streams\nAcme / Checkout\n- Content logging is off\nAnd 3 more changes.\n\nRead the release notes:\nhttps://example.com/notes\n"), "{text}");
    }
}
