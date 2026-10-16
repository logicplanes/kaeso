use time::Date;

use crate::layout::{Content, Instance, Reason};
use crate::locale::{Locale, zh};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmailKind {
    Invitation,
    InstanceInvitation,
    MagicLink,
    PasswordReset,
    EmailChangeConfirmation,
    Signup,
    PasswordChanged,
    TwoFactorEnabled,
    TwoFactorDisabled,
    TwoFactorReset,
    EmailChanged,
    SessionsEnded,
    AccountDeactivated,
    OrgScheduledForDeletion,
    OwnershipTransferred,
    KeyExpiryWarning,
    BudgetAlert,
    Test,
}

impl EmailKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EmailKind::Invitation => "invitation",
            EmailKind::InstanceInvitation => "instance_invitation",
            EmailKind::MagicLink => "magic_link",
            EmailKind::PasswordReset => "password_reset",
            EmailKind::EmailChangeConfirmation => "email_change_confirmation",
            EmailKind::Signup => "signup",
            EmailKind::PasswordChanged => "password_changed",
            EmailKind::TwoFactorEnabled => "two_factor_enabled",
            EmailKind::TwoFactorDisabled => "two_factor_disabled",
            EmailKind::TwoFactorReset => "two_factor_reset",
            EmailKind::EmailChanged => "email_changed",
            EmailKind::SessionsEnded => "sessions_ended",
            EmailKind::AccountDeactivated => "account_deactivated",
            EmailKind::OrgScheduledForDeletion => "org_scheduled_for_deletion",
            EmailKind::OwnershipTransferred => "ownership_transferred",
            EmailKind::KeyExpiryWarning => "key_expiry_warning",
            EmailKind::BudgetAlert => "budget_alert",
            EmailKind::Test => "test",
        }
    }

    pub fn is_alert(self) -> bool {
        matches!(self, EmailKind::KeyExpiryWarning | EmailKind::BudgetAlert)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Email {
    pub to: String,
    pub kind: EmailKind,
    pub lang: Locale,
    pub subject: String,
    content: Content,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    pub to: String,
    pub kind: EmailKind,
    pub lang: Locale,
    pub subject: String,
    pub text: String,
    pub html: String,
}

impl Email {
    fn new(to: &str, kind: EmailKind, subject: String, content: Content) -> Email {
        Email {
            to: to.to_owned(),
            kind,
            lang: content.lang,
            subject,
            content,
        }
    }

    pub fn render(&self, instance: &Instance) -> Rendered {
        Rendered {
            to: self.to.clone(),
            kind: self.kind,
            lang: self.lang,
            subject: self.subject.clone(),
            text: crate::layout::text(&self.content, instance),
            html: crate::layout::html(&self.subject, &self.content, instance),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor<'a> {
    Person(&'a str),
    Member,
    Owner,
    OrgAdmin(&'a str),
    Administrator,
    InstallAdministrator,
    Operator,
}

impl<'a> From<&'a str> for Actor<'a> {
    fn from(name: &'a str) -> Actor<'a> {
        Actor::Person(name)
    }
}

impl<'a> From<&'a String> for Actor<'a> {
    fn from(name: &'a String) -> Actor<'a> {
        Actor::Person(name)
    }
}

impl Actor<'_> {
    fn name(self, lang: Locale) -> String {
        match (lang, self) {
            (_, Actor::Person(name)) => name.to_owned(),
            (Locale::En, Actor::Member) => "A member".to_owned(),
            (Locale::En, Actor::Owner) => "An owner".to_owned(),
            (Locale::En, Actor::OrgAdmin(org)) => format!("An admin of {org}"),
            (Locale::En, Actor::Administrator) => "An administrator".to_owned(),
            (Locale::En, Actor::InstallAdministrator) => {
                "An administrator of this Kaeso install".to_owned()
            }
            (Locale::En, Actor::Operator) => "The operator of this Kaeso install".to_owned(),
            (Locale::De, Actor::Member) => "Ein Mitglied".to_owned(),
            (Locale::De, Actor::Owner) => "Ein Inhaber".to_owned(),
            (Locale::De, Actor::OrgAdmin(org)) => format!("Ein Admin von {org}"),
            (Locale::De, Actor::Administrator) => "Ein Admin".to_owned(),
            (Locale::De, Actor::InstallAdministrator) => {
                "Ein Admin dieser Kaeso-Instanz".to_owned()
            }
            (Locale::De, Actor::Operator) => "Der Betreiber dieser Kaeso-Instanz".to_owned(),
            (Locale::ZhCn, Actor::Member) => "一名成员".to_owned(),
            (Locale::ZhCn, Actor::Owner) => "一名所有者".to_owned(),
            (Locale::ZhCn, Actor::OrgAdmin(org)) => zh(&[org, "的管理员"]),
            (Locale::ZhCn, Actor::Administrator) => "一名管理员".to_owned(),
            (Locale::ZhCn, Actor::InstallAdministrator) => "此 Kaeso 实例的管理员".to_owned(),
            (Locale::ZhCn, Actor::Operator) => "此 Kaeso 实例的运营方".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind {
    RequestRate,
    TokenRate,
    TokenBudget,
    SpendBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitPeriod {
    Minute,
    Day,
    Month,
    Total,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    pub kind: LimitKind,
    pub period: LimitPeriod,
    pub amount: String,
}

impl Budget {
    pub fn english(&self) -> String {
        let n = &self.amount;
        let amount = match self.kind {
            LimitKind::RequestRate => format!("{n} requests per minute"),
            LimitKind::TokenRate => format!("{n} tokens per minute"),
            LimitKind::TokenBudget => format!("{n} tokens"),
            LimitKind::SpendBudget => format!("{n} USD"),
        };
        let label = match self.kind {
            LimitKind::RequestRate => "request rate",
            LimitKind::TokenRate => "token rate",
            LimitKind::TokenBudget => "token budget",
            LimitKind::SpendBudget => "spend budget",
        };
        let period = match (self.kind, self.period) {
            (LimitKind::RequestRate | LimitKind::TokenRate, _) => "",
            (_, LimitPeriod::Minute) => "per-minute ",
            (_, LimitPeriod::Day) => "daily ",
            (_, LimitPeriod::Month) => "monthly ",
            (_, LimitPeriod::Total) => "total ",
        };
        format!("{period}{label} of {amount}")
    }

    fn german_genitive(&self) -> String {
        let n = Locale::De.number(&self.amount);
        let budget = match self.period {
            LimitPeriod::Day => "Tagesbudgets",
            LimitPeriod::Month => "Monatsbudgets",
            LimitPeriod::Total => "Gesamtbudgets",
            LimitPeriod::Minute => "Budgets",
        };
        match self.kind {
            LimitKind::RequestRate => format!("Ratenlimits von {n} Anfragen pro Minute"),
            LimitKind::TokenRate => format!("Ratenlimits von {n} Tokens pro Minute"),
            LimitKind::TokenBudget => format!("{budget} von {n} Tokens"),
            LimitKind::SpendBudget => format!("{budget} von {n} USD"),
        }
    }

    fn chinese(&self) -> String {
        let n = Locale::ZhCn.number(&self.amount);
        match (self.kind, self.period) {
            (LimitKind::RequestRate, _) => format!("速率限制（每分钟 {n} 次请求）"),
            (LimitKind::TokenRate, _) => format!("速率限制（每分钟 {n} 个 Token）"),
            (LimitKind::TokenBudget, LimitPeriod::Day) => {
                format!("每日 Token 预算（{n} 个 Token）")
            }
            (LimitKind::TokenBudget, LimitPeriod::Month) => {
                format!("每月 Token 预算（{n} 个 Token）")
            }
            (LimitKind::TokenBudget, _) => format!("Token 总预算（{n} 个 Token）"),
            (LimitKind::SpendBudget, LimitPeriod::Day) => format!("每日支出预算（{n} USD）"),
            (LimitKind::SpendBudget, LimitPeriod::Month) => format!("每月支出预算（{n} USD）"),
            (LimitKind::SpendBudget, _) => format!("总支出预算（{n} USD）"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetScope<'a> {
    Project,
    Member,
    Key(&'a str),
}

const NOT_YOU: &str =
    "If this was not you, reset your password and tell your organization's admin.";

fn not_you(lang: Locale) -> &'static str {
    match lang {
        Locale::En => NOT_YOU,
        Locale::De => {
            "Waren Sie das nicht, setzen Sie Ihr Passwort zurück und informieren Sie den Admin Ihrer Organisation."
        }
        Locale::ZhCn => "如果这不是您本人的操作，请重置密码并告知您组织的管理员。",
    }
}

fn invitation_note(lang: Locale) -> &'static str {
    match lang {
        Locale::En => {
            "The invitation expires in 21 days; if you did not expect it, ignore this email."
        }
        Locale::De => {
            "Die Einladung gilt 21 Tage; wenn Sie sie nicht erwartet haben, ignorieren Sie diese E-Mail."
        }
        Locale::ZhCn => "邀请 21 天后过期；如果您没有预料到此邀请，请忽略此邮件。",
    }
}

pub fn invitation<'a>(
    lang: Locale,
    to: &str,
    inviter: impl Into<Actor<'a>>,
    org: &str,
    role: &str,
    url: &str,
) -> Email {
    let inviter = inviter.into().name(lang);
    let inviter = inviter.as_str();
    let (subject, heading, sentence, action) = match lang {
        Locale::En => (
            format!("{inviter} invited you to {org} on Kaeso"),
            format!("Join {org} on Kaeso"),
            format!("{inviter} invited you to join {}.", as_role(role)),
            "Accept invitation",
        ),
        Locale::De => (
            format!("{inviter} hat Sie zu {org} auf Kaeso eingeladen"),
            format!("{org} auf Kaeso beitreten"),
            format!("{inviter} hat Sie eingeladen, {}.", als_rolle(role)),
            "Einladung annehmen",
        ),
        Locale::ZhCn => (
            zh(&[inviter, "邀请您加入 Kaeso 上的", org]),
            zh(&["加入 Kaeso 上的", org]),
            zh(&[inviter, "邀请您", &zh_role(role), "。"]),
            "接受邀请",
        ),
    };
    Email::new(
        to,
        EmailKind::Invitation,
        subject,
        Content::new(lang, heading, Reason::Request)
            .sentence(sentence)
            .action(action, url)
            .note(invitation_note(lang)),
    )
}

fn as_role(preset: &str) -> String {
    match preset {
        "owner" => "as an owner".to_owned(),
        "admin" => "as an admin".to_owned(),
        "developer" => "as a developer".to_owned(),
        "member" => "as a member".to_owned(),
        "auditor" => "as an auditor".to_owned(),
        "billing" => "with billing access".to_owned(),
        other => format!("as {other}"),
    }
}

fn als_rolle(preset: &str) -> String {
    match preset {
        "owner" => "als Inhaber beizutreten".to_owned(),
        "admin" => "als Admin beizutreten".to_owned(),
        "developer" => "als Entwickler beizutreten".to_owned(),
        "member" => "als Mitglied beizutreten".to_owned(),
        "auditor" => "als Prüfer beizutreten".to_owned(),
        "billing" => "mit Zugriff auf die Abrechnung beizutreten".to_owned(),
        other => format!("als {other} beizutreten"),
    }
}

fn zh_role(preset: &str) -> String {
    match preset {
        "owner" => "以所有者身份加入".to_owned(),
        "admin" => "以管理员身份加入".to_owned(),
        "developer" => "以开发者身份加入".to_owned(),
        "member" => "以成员身份加入".to_owned(),
        "auditor" => "以审计员身份加入".to_owned(),
        "billing" => "加入并管理账单".to_owned(),
        other => zh(&["以", other, "身份加入"]),
    }
}

pub fn instance_invitation<'a>(
    lang: Locale,
    to: &str,
    inviter: impl Into<Actor<'a>>,
    url: &str,
) -> Email {
    let inviter = inviter.into().name(lang);
    let inviter = inviter.as_str();
    let (subject, heading, sentence, action) = match lang {
        Locale::En => (
            format!("{inviter} invited you to Kaeso"),
            "Join Kaeso",
            format!("{inviter} invited you to create an account on this install."),
            "Create account",
        ),
        Locale::De => (
            format!("{inviter} hat Sie zu Kaeso eingeladen"),
            "Kaeso beitreten",
            format!("{inviter} hat Sie eingeladen, ein Konto auf dieser Instanz zu erstellen."),
            "Konto erstellen",
        ),
        Locale::ZhCn => (
            zh(&[inviter, "邀请您加入 Kaeso"]),
            "加入 Kaeso",
            zh(&[inviter, "邀请您在此实例上创建账户。"]),
            "创建账户",
        ),
    };
    Email::new(
        to,
        EmailKind::InstanceInvitation,
        subject,
        Content::new(lang, heading, Reason::Request)
            .sentence(sentence)
            .action(action, url)
            .note(invitation_note(lang)),
    )
}

pub fn magic_link(lang: Locale, to: &str, url: &str) -> Email {
    let (subject, sentence, action, note) = match lang {
        Locale::En => (
            "Sign in to Kaeso",
            "The link works once and expires in 15 minutes.",
            "Sign in",
            "If you did not ask to sign in, you can ignore this email.",
        ),
        Locale::De => (
            "Bei Kaeso anmelden",
            "Der Link funktioniert einmal und läuft in 15 Minuten ab.",
            "Anmelden",
            "Wenn Sie keine Anmeldung angefordert haben, können Sie diese E-Mail ignorieren.",
        ),
        Locale::ZhCn => (
            "登录 Kaeso",
            "此链接仅可使用一次，15 分钟后过期。",
            "登录",
            "如果您没有请求登录，可以忽略此邮件。",
        ),
    };
    Email::new(
        to,
        EmailKind::MagicLink,
        subject.to_owned(),
        Content::new(lang, subject, Reason::Request)
            .sentence(sentence)
            .action(action, url)
            .note(note),
    )
}

pub fn password_reset(lang: Locale, to: &str, url: &str) -> Email {
    let (subject, heading, sentence, action, note) = match lang {
        Locale::En => (
            "Reset your Kaeso password",
            "Reset your password",
            "The link works once and expires in 15 minutes; a new password signs you out everywhere.",
            "Choose new password",
            "If you did not ask for a reset, you can ignore this email.",
        ),
        Locale::De => (
            "Kaeso-Passwort zurücksetzen",
            "Passwort zurücksetzen",
            "Der Link funktioniert einmal und läuft in 15 Minuten ab; ein neues Passwort meldet Sie überall ab.",
            "Neues Passwort wählen",
            "Wenn Sie das nicht angefordert haben, können Sie diese E-Mail ignorieren.",
        ),
        Locale::ZhCn => (
            "重置您的 Kaeso 密码",
            "重置密码",
            "此链接仅可使用一次，15 分钟后过期；设置新密码后，所有设备都会退出登录。",
            "设置新密码",
            "如果您没有请求重置，可以忽略此邮件。",
        ),
    };
    Email::new(
        to,
        EmailKind::PasswordReset,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Request)
            .sentence(sentence)
            .action(action, url)
            .note(note),
    )
}

pub fn email_change_confirmation(lang: Locale, to: &str, url: &str) -> Email {
    let (subject, heading, sentence, action, note) = match lang {
        Locale::En => (
            "Confirm your new Kaeso email address",
            "Confirm your new address",
            format!("Once you confirm, your Kaeso account uses {to}."),
            "Confirm address",
            "The link expires in 24 hours; if you did not ask for this, ignore this email.",
        ),
        Locale::De => (
            "Neue E-Mail-Adresse für Kaeso bestätigen",
            "Neue Adresse bestätigen",
            format!("Nach der Bestätigung verwendet Ihr Kaeso-Konto {to}."),
            "Adresse bestätigen",
            "Der Link läuft in 24 Stunden ab; wenn Sie das nicht angefordert haben, ignorieren Sie diese E-Mail.",
        ),
        Locale::ZhCn => (
            "确认您的新 Kaeso 邮箱地址",
            "确认新地址",
            zh(&["确认后，您的 Kaeso 账户将使用", to, "。"]),
            "确认地址",
            "此链接 24 小时后过期；如果这不是您的请求，请忽略此邮件。",
        ),
    };
    Email::new(
        to,
        EmailKind::EmailChangeConfirmation,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Request)
            .sentence(sentence)
            .action(action, url)
            .note(note),
    )
}

pub fn signup(lang: Locale, to: &str, url: &str) -> Email {
    let (subject, heading, sentence, action, note) = match lang {
        Locale::En => (
            "Finish creating your Kaeso account",
            "Create your Kaeso account",
            "The link works once and expires in 24 hours.",
            "Create account",
            "If you did not sign up, you can ignore this email.",
        ),
        Locale::De => (
            "Kaeso-Konto fertig erstellen",
            "Kaeso-Konto erstellen",
            "Der Link funktioniert einmal und läuft in 24 Stunden ab.",
            "Konto erstellen",
            "Wenn Sie sich nicht registriert haben, können Sie diese E-Mail ignorieren.",
        ),
        Locale::ZhCn => (
            "完成创建您的 Kaeso 账户",
            "创建 Kaeso 账户",
            "此链接仅可使用一次，24 小时后过期。",
            "创建账户",
            "如果您没有注册，可以忽略此邮件。",
        ),
    };
    Email::new(
        to,
        EmailKind::Signup,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Request)
            .sentence(sentence)
            .action(action, url)
            .note(note),
    )
}

pub fn password_changed(lang: Locale, to: &str) -> Email {
    let (subject, heading, sentence) = match lang {
        Locale::En => (
            "Your Kaeso password was changed",
            "Your password was changed",
            "Every session of your account was signed out.",
        ),
        Locale::De => (
            "Ihr Kaeso-Passwort wurde geändert",
            "Ihr Passwort wurde geändert",
            "Alle Sitzungen Ihres Kontos wurden abgemeldet.",
        ),
        Locale::ZhCn => (
            "您的 Kaeso 密码已更改",
            "密码已更改",
            "您账户的所有会话均已退出登录。",
        ),
    };
    Email::new(
        to,
        EmailKind::PasswordChanged,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Security)
            .sentence(sentence)
            .note(not_you(lang)),
    )
}

pub fn two_factor_enabled(lang: Locale, to: &str) -> Email {
    let (subject, sentence) = match lang {
        Locale::En => (
            "Two-factor authentication is on",
            "Keep your recovery codes safe; they sign you in if you lose your authenticator.",
        ),
        Locale::De => (
            "Zwei-Faktor-Authentifizierung ist aktiviert",
            "Bewahren Sie Ihre Wiederherstellungscodes sicher auf; sie ersetzen einen verlorenen Authenticator.",
        ),
        Locale::ZhCn => (
            "双重验证已开启",
            "请妥善保管恢复码；丢失验证器时，可以用它们登录。",
        ),
    };
    Email::new(
        to,
        EmailKind::TwoFactorEnabled,
        subject.to_owned(),
        Content::new(lang, subject, Reason::Security)
            .sentence(sentence)
            .note(not_you(lang)),
    )
}

pub fn two_factor_disabled(lang: Locale, to: &str) -> Email {
    let (subject, sentence) = match lang {
        Locale::En => (
            "Two-factor authentication is off",
            "Signing in to your account no longer asks for a code.",
        ),
        Locale::De => (
            "Zwei-Faktor-Authentifizierung ist deaktiviert",
            "Bei der Anmeldung an Ihrem Konto wird kein Code mehr abgefragt.",
        ),
        Locale::ZhCn => ("双重验证已关闭", "登录您的账户时不再需要验证码。"),
    };
    Email::new(
        to,
        EmailKind::TwoFactorDisabled,
        subject.to_owned(),
        Content::new(lang, subject, Reason::Security)
            .sentence(sentence)
            .note(not_you(lang)),
    )
}

pub fn two_factor_reset<'a>(lang: Locale, to: &str, by: impl Into<Actor<'a>>) -> Email {
    let by = by.into().name(lang);
    let (subject, heading, sentence, note) = match lang {
        Locale::En => (
            "Your two-factor authentication was reset",
            "Two-factor authentication was reset",
            format!("{by} reset it and signed you out; set it up again when you next sign in."),
            "If you did not ask for this, contact your organization's admin.",
        ),
        Locale::De => (
            "Ihre Zwei-Faktor-Authentifizierung wurde zurückgesetzt",
            "Zwei-Faktor-Authentifizierung zurückgesetzt",
            format!(
                "{by} hat sie zurückgesetzt und Sie abgemeldet; richten Sie sie bei der nächsten Anmeldung neu ein."
            ),
            "Wenn Sie das nicht angefordert haben, wenden Sie sich an den Admin Ihrer Organisation.",
        ),
        Locale::ZhCn => (
            "您的双重验证已被重置",
            "双重验证已重置",
            zh(&[
                &by,
                "重置了您的双重验证并让您退出登录；请在下次登录时重新设置。",
            ]),
            "如果这不是您的请求，请联系您组织的管理员。",
        ),
    };
    Email::new(
        to,
        EmailKind::TwoFactorReset,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Security)
            .sentence(sentence)
            .note(note),
    )
}

pub fn email_changed(lang: Locale, to: &str, new_address: &str) -> Email {
    let (subject, heading, sentence) = match lang {
        Locale::En => (
            "Your Kaeso email address was changed",
            "Your email address was changed",
            format!("Your account now uses {new_address}; Kaeso no longer sends email here."),
        ),
        Locale::De => (
            "Ihre E-Mail-Adresse für Kaeso wurde geändert",
            "Ihre E-Mail-Adresse wurde geändert",
            format!(
                "Ihr Konto verwendet jetzt {new_address}; Kaeso sendet keine E-Mails mehr an diese Adresse."
            ),
        ),
        Locale::ZhCn => (
            "您的 Kaeso 邮箱地址已更改",
            "邮箱地址已更改",
            zh(&[
                "您的账户现在使用",
                new_address,
                "；Kaeso 不会再向此地址发送邮件。",
            ]),
        ),
    };
    Email::new(
        to,
        EmailKind::EmailChanged,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Security)
            .sentence(sentence)
            .note(not_you(lang)),
    )
}

pub fn sessions_ended<'a>(lang: Locale, to: &str, by: impl Into<Actor<'a>>) -> Email {
    let by = by.into().name(lang);
    let (subject, heading, sentence) = match lang {
        Locale::En => (
            "You were signed out of Kaeso",
            "You were signed out",
            format!("{by} ended every session of your account; sign in again to continue."),
        ),
        Locale::De => (
            "Sie wurden von Kaeso abgemeldet",
            "Sie wurden abgemeldet",
            format!("{by} hat alle Sitzungen Ihres Kontos beendet; melden Sie sich erneut an."),
        ),
        Locale::ZhCn => (
            "您已退出 Kaeso",
            "您已退出登录",
            zh(&[&by, "结束了您账户的所有会话；请重新登录以继续。"]),
        ),
    };
    Email::new(
        to,
        EmailKind::SessionsEnded,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Security).sentence(sentence),
    )
}

pub fn account_deactivated(lang: Locale, to: &str) -> Email {
    let (subject, heading, sentence, note) = match lang {
        Locale::En => (
            "Your Kaeso account was deactivated",
            "Your account was deactivated",
            "An administrator signed you out, and your API keys stopped working.",
            "If you think this is a mistake, contact the administrator.",
        ),
        Locale::De => (
            "Ihr Kaeso-Konto wurde deaktiviert",
            "Ihr Konto wurde deaktiviert",
            "Ein Admin hat Sie abgemeldet, und Ihre API-Schlüssel funktionieren nicht mehr.",
            "Wenn Sie das für einen Fehler halten, wenden Sie sich an den Admin.",
        ),
        Locale::ZhCn => (
            "您的 Kaeso 账户已停用",
            "账户已停用",
            "管理员已让您退出登录，您的 API 密钥也已停止工作。",
            "如果您认为这是误操作，请联系管理员。",
        ),
    };
    Email::new(
        to,
        EmailKind::AccountDeactivated,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Security)
            .sentence(sentence)
            .note(note),
    )
}

pub fn org_scheduled_for_deletion<'a>(
    lang: Locale,
    to: &str,
    org: &str,
    by: impl Into<Actor<'a>>,
    due: Date,
    url: &str,
) -> Email {
    let by = by.into().name(lang);
    let due = lang.date(due);
    let (subject, heading, sentence, action, note) = match lang {
        Locale::En => (
            format!("{org} is scheduled for deletion"),
            format!("{org} will be deleted on {due}"),
            format!("{by} scheduled it for deletion; its API keys stopped working."),
            "Open organization",
            "Until then, an owner or an instance admin can restore it.",
        ),
        Locale::De => (
            format!("{org} ist zur Löschung vorgemerkt"),
            format!("{org} wird am {due} gelöscht"),
            format!(
                "{by} hat sie zur Löschung vorgemerkt; ihre API-Schlüssel funktionieren nicht mehr."
            ),
            "Organisation öffnen",
            "Bis dahin kann ein Inhaber oder ein Instanz-Admin sie wiederherstellen.",
        ),
        Locale::ZhCn => (
            zh(&[org, "已计划删除"]),
            zh(&[org, "将于", &due, "删除"]),
            zh(&[&by, "已计划删除该组织；其 API 密钥已停止工作。"]),
            "打开组织",
            "在此之前，所有者或实例管理员可以恢复它。",
        ),
    };
    Email::new(
        to,
        EmailKind::OrgScheduledForDeletion,
        subject,
        Content::new(lang, heading, Reason::Org)
            .sentence(sentence)
            .action(action, url)
            .note(note),
    )
}

pub fn ownership_transferred<'a>(
    lang: Locale,
    to: &str,
    org: &str,
    by: impl Into<Actor<'a>>,
    url: &str,
) -> Email {
    let by = by.into().name(lang);
    let (subject, sentence, action) = match lang {
        Locale::En => (
            format!("You are now an owner of {org}"),
            format!("{by} made you an owner of the organization."),
            "Open organization",
        ),
        Locale::De => (
            format!("Sie sind jetzt Inhaber von {org}"),
            format!("{by} hat Sie zum Inhaber der Organisation gemacht."),
            "Organisation öffnen",
        ),
        Locale::ZhCn => (
            zh(&["您现在是", org, "的所有者"]),
            zh(&[&by, "已将您设为该组织的所有者。"]),
            "打开组织",
        ),
    };
    Email::new(
        to,
        EmailKind::OwnershipTransferred,
        subject.clone(),
        Content::new(lang, subject, Reason::Org)
            .sentence(sentence)
            .action(action, url),
    )
}

pub fn key_expiry_warning(
    lang: Locale,
    to: &str,
    key: &str,
    project: &str,
    expires: Date,
    url: &str,
) -> Email {
    let expires = lang.date(expires);
    let (subject, heading, sentence, action, note) = match lang {
        Locale::En => (
            format!("The API key {key} expires on {expires}"),
            format!("API key {key} expires on {expires}"),
            format!("Requests with this key in {project} fail after that day."),
            "Open API keys",
            "To rotate it, create a new key, deploy it, then disable the old one.",
        ),
        Locale::De => (
            format!("Der API-Schlüssel {key} läuft am {expires} ab"),
            format!("API-Schlüssel {key} läuft am {expires} ab"),
            format!("Anfragen mit diesem Schlüssel in {project} schlagen nach diesem Tag fehl."),
            "API-Schlüssel öffnen",
            "Zum Rotieren erstellen Sie einen neuen Schlüssel, setzen ihn ein und deaktivieren den alten.",
        ),
        Locale::ZhCn => {
            let heading = zh(&["API 密钥", key, "将于", &expires, "过期"]);
            (
                heading.clone(),
                heading,
                zh(&["在", project, "中使用此密钥的请求，将在该日期之后失败。"]),
                "打开 API 密钥",
                "如需轮换，请创建并部署新密钥，再停用旧密钥。",
            )
        }
    };
    Email::new(
        to,
        EmailKind::KeyExpiryWarning,
        subject,
        Content::new(lang, heading, Reason::Alert)
            .sentence(sentence)
            .action(action, url)
            .note(note),
    )
}

pub fn budget_alert(
    lang: Locale,
    to: &str,
    budget: &Budget,
    scope: BudgetScope<'_>,
    percent: u32,
    url: &str,
) -> Email {
    let p = lang.percent(percent);
    let (subject, heading, sentence, action) = match lang {
        Locale::En => {
            let (budget, scope) = (budget.english(), scope_en(scope));
            (
                format!("{scope} used {p} of its {budget}"),
                format!("A budget is {p} used"),
                format!("{scope} has used {p} of its {budget}."),
                "Open limits",
            )
        }
        Locale::De => {
            let scope = match scope {
                BudgetScope::Project => "Das Projekt".to_owned(),
                BudgetScope::Member => "Ein Mitglied des Projekts".to_owned(),
                BudgetScope::Key(id) => format!("Der API-Schlüssel {id}"),
            };
            let line = format!(
                "{scope} hat {p} des {} verbraucht",
                budget.german_genitive()
            );
            (
                line.clone(),
                format!("Ein Budget ist zu {p} verbraucht"),
                format!("{line}."),
                "Limits öffnen",
            )
        }
        Locale::ZhCn => {
            let whose = match scope {
                BudgetScope::Project => "该项目的".to_owned(),
                BudgetScope::Member => "项目中一名成员的".to_owned(),
                BudgetScope::Key(id) => zh(&["API 密钥", id, "的"]),
            };
            let line = zh(&[&whose, &budget.chinese(), "已用", &p]);
            (
                line.clone(),
                zh(&["预算已用", &p]),
                format!("{line}。"),
                "打开限额",
            )
        }
    };
    Email::new(
        to,
        EmailKind::BudgetAlert,
        subject,
        Content::new(lang, heading, Reason::Alert)
            .sentence(sentence)
            .action(action, url),
    )
}

fn scope_en(scope: BudgetScope<'_>) -> String {
    match scope {
        BudgetScope::Project => "The project".to_owned(),
        BudgetScope::Member => "A member of the project".to_owned(),
        BudgetScope::Key(id) => format!("The API key {id}"),
    }
}

pub fn test(lang: Locale, to: &str) -> Email {
    let (subject, heading, sentence) = match lang {
        Locale::En => (
            "Kaeso test email",
            "Email works",
            "This test from the instance settings arrived, so Kaeso can send email.",
        ),
        Locale::De => (
            "Kaeso-Test-E-Mail",
            "E-Mail funktioniert",
            "Dieser Test aus den Instanzeinstellungen ist angekommen, also kann Kaeso E-Mails senden.",
        ),
        Locale::ZhCn => (
            "Kaeso 测试邮件",
            "邮件发送正常",
            "这封来自实例设置的测试邮件已送达，说明 Kaeso 可以发送邮件。",
        ),
    };
    Email::new(
        to,
        EmailKind::Test,
        subject.to_owned(),
        Content::new(lang, heading, Reason::Request).sentence(sentence),
    )
}

#[cfg(test)]
mod tests {

    use super::*;
    use time::macros::date;

    const TO: &str = "owner@example.com";
    const URL: &str =
        "https://kaeso.example.com/auth/magic-link#token=kGq3VbX9yZ2mN7pR4sT8uW1aC5dE6fH0jK";

    fn instance() -> Instance {
        Instance::new("https://kaeso.example.com/")
    }

    fn monthly_spend(amount: &str) -> Budget {
        Budget {
            kind: LimitKind::SpendBudget,
            period: LimitPeriod::Month,
            amount: amount.to_owned(),
        }
    }

    fn all() -> Vec<Email> {
        all_in(Locale::En)
    }

    fn all_in(lang: Locale) -> Vec<Email> {
        let l = lang;
        vec![
            invitation(l, TO, "Ada Lovelace", "Acme", "developer", URL),
            instance_invitation(l, TO, "Ada Lovelace", URL),
            magic_link(l, TO, URL),
            password_reset(l, TO, URL),
            email_change_confirmation(l, TO, URL),
            signup(l, TO, URL),
            password_changed(l, TO),
            two_factor_enabled(l, TO),
            two_factor_disabled(l, TO),
            two_factor_reset(l, TO, Actor::Administrator),
            email_changed(l, TO, "new@example.com"),
            sessions_ended(l, TO, Actor::Administrator),
            account_deactivated(l, TO),
            org_scheduled_for_deletion(l, TO, "Acme", "Ada Lovelace", date!(2026 - 11 - 10), URL),
            ownership_transferred(l, TO, "Acme", "Ada Lovelace", URL),
            key_expiry_warning(
                l,
                TO,
                "ci-deploy",
                "Default project",
                date!(2026 - 10 - 18),
                URL,
            ),
            budget_alert(l, TO, &monthly_spend("50"), BudgetScope::Project, 80, URL),
            test(l, TO),
        ]
    }

    fn visible_text(html: &str) -> String {
        let mut rest = html;
        let mut out = String::new();
        while let Some(i) = rest.find("<style") {
            out.push_str(&rest[..i]);
            rest = &rest[rest.find("</style>").unwrap() + 8..];
        }
        out.push_str(rest);
        let mut text = String::new();
        let (mut in_tag, mut in_ref) = (false, false);
        for c in out.chars() {
            match c {
                '<' => in_tag = true,
                '>' if in_tag => {
                    in_tag = false;
                    text.push(' ');
                }
                '&' if !in_tag => in_ref = true,
                ';' if in_ref => {
                    in_ref = false;
                    text.push(' ');
                }
                c if !in_tag && !in_ref => text.push(c),
                _ => {}
            }
        }
        text
    }

    #[test]
    fn every_email_renders() {
        for email in all() {
            let r = email.render(&instance());
            insta::assert_snapshot!(
                format!("{}_text", email.kind.as_str()),
                format!("Subject: {}\n\n{}", r.subject, r.text)
            );
            insta::assert_snapshot!(format!("{}_html", email.kind.as_str()), r.html);
        }
    }

    #[test]
    fn every_kind_has_an_email() {
        let kinds: Vec<_> = all().iter().map(|e| e.kind.as_str()).collect();
        for kind in [
            EmailKind::Invitation,
            EmailKind::InstanceInvitation,
            EmailKind::MagicLink,
            EmailKind::PasswordReset,
            EmailKind::EmailChangeConfirmation,
            EmailKind::Signup,
            EmailKind::PasswordChanged,
            EmailKind::TwoFactorEnabled,
            EmailKind::TwoFactorDisabled,
            EmailKind::TwoFactorReset,
            EmailKind::EmailChanged,
            EmailKind::SessionsEnded,
            EmailKind::AccountDeactivated,
            EmailKind::OrgScheduledForDeletion,
            EmailKind::OwnershipTransferred,
            EmailKind::KeyExpiryWarning,
            EmailKind::BudgetAlert,
            EmailKind::Test,
        ] {
            assert!(kinds.contains(&kind.as_str()), "{}", kind.as_str());
        }
    }

    #[test]
    fn every_email_has_both_parts_and_a_controlled_dark_mode() {
        for email in all() {
            let r = email.render(&instance());
            let kind = email.kind.as_str();
            assert!(r.text.starts_with(&email.content.heading), "{kind}");
            assert!(r.html.starts_with("<!DOCTYPE html>"), "{kind}");
            for needle in [
                r#"<meta name="color-scheme" content="light dark">"#,
                r#"<meta name="supported-color-schemes" content="light dark">"#,
                ":root{color-scheme:light dark;supported-color-schemes:light dark}",
                "@media (prefers-color-scheme:dark){",
                "[data-ogsc] .k-text{",
                "[data-ogsb] .k-bg{",
                r#"src="cid:wordmark-light@kaeso""#,
                r#"src="cid:wordmark-dark@kaeso""#,
                r#"alt="Kaeso""#,
                "Sent by Kaeso at",
            ] {
                assert!(r.html.contains(needle), "{kind}: {needle}");
            }
            for banned in ["<link", "<script", "@import", "@font-face", "src=\"http"] {
                assert!(!r.html.contains(banned), "{kind}: {banned}");
            }
            assert!(r.html.len() < 30 * 1024, "{kind}: {}", r.html.len());
        }
    }

    #[test]
    fn buttons_link_without_showing_the_url() {
        for email in all() {
            let r = email.render(&instance());
            let kind = email.kind.as_str();
            let visible = visible_text(&r.html);
            assert!(!visible.contains("://"), "{kind}: {visible}");
            assert!(!visible.contains("#token"), "{kind}");
            for word in visible.split_whitespace() {
                assert!(word.chars().count() <= 32, "{kind}: {word}");
            }
            if let Some(action) = &email.content.action {
                let href = format!("href=\"{}\"", action.url);
                assert_eq!(r.html.matches(&href).count(), 2, "{kind}");
                assert!(r.text.contains(&format!("\n{}\n", action.url)), "{kind}");
            }
        }
    }

    #[test]
    fn copy_is_short() {
        for email in all() {
            let c = &email.content;
            let kind = email.kind.as_str();
            assert!(
                email.subject.chars().count() <= 60,
                "{kind}: {}",
                email.subject
            );
            assert!(!email.subject.ends_with('.'), "{kind}");
            assert!(c.heading.chars().count() <= 48, "{kind}: {}", c.heading);
            assert!(!c.heading.ends_with('.'), "{kind}");
            for line in [&c.sentence, &c.note].into_iter().flatten() {
                assert!(line.chars().count() <= 100, "{kind}: {line}");
                assert!(line.ends_with('.'), "{kind}: {line}");
                assert!(!line.trim_end_matches('.').contains(". "), "{kind}: {line}");
            }
        }
    }

    #[test]
    fn names_cannot_inject_markup() {
        let evil = r#"<img src=x onerror=alert(1)> & "quoted" 'single'"#;
        let day = date!(2026 - 11 - 10);
        for email in Locale::ALL.into_iter().flat_map(|l| {
            [
                invitation(l, TO, evil, evil, evil, URL),
                instance_invitation(l, TO, evil, URL),
                two_factor_reset(l, TO, evil),
                email_changed(l, TO, evil),
                sessions_ended(l, TO, evil),
                org_scheduled_for_deletion(l, TO, evil, evil, day, URL),
                ownership_transferred(l, TO, evil, evil, URL),
                key_expiry_warning(l, TO, evil, evil, day, URL),
                budget_alert(l, TO, &monthly_spend(evil), BudgetScope::Key(evil), 80, URL),
            ]
        }) {
            let r = email.render(&instance());
            let kind = email.kind.as_str();
            assert!(!r.html.contains("<img src=x"), "{kind}");
            assert!(!r.html.contains(r#""quoted""#), "{kind}");
            assert!(
                r.html
                    .contains("&lt;img src=x onerror=alert(1)&gt; &amp; &quot;quoted&quot;"),
                "{kind}"
            );
            assert!(r.text.contains(evil), "{kind}");
        }
    }

    #[test]
    fn a_hostile_url_stays_inside_its_attribute() {
        let url = r#"https://kaeso.example.com/x"><script>alert(1)</script><a href=""#;
        let r = magic_link(Locale::En, TO, url).render(&instance());
        assert!(!r.html.contains("<script>"));
        assert!(!r.html.contains(r#"x"><"#));
    }

    #[test]
    fn the_footer_names_the_install_and_why() {
        let r = budget_alert(
            Locale::En,
            TO,
            &monthly_spend("50"),
            BudgetScope::Project,
            80,
            URL,
        )
        .render(&Instance::new("http://localhost:3000/"));
        assert!(r.text.ends_with(
            "Sent by Kaeso at localhost:3000; turn alerts off in your account's notification settings.\n"
        ));
        assert!(r.html.contains(r#"<a href="http://localhost:3000/""#));
        let r = password_changed(Locale::En, TO).render(&instance());
        assert!(r.text.contains("security notices are always sent."));
    }

    #[test]
    fn invitations_name_the_preset_in_words() {
        let sentence = |preset| {
            invitation(Locale::En, TO, "Ada", "Acme", preset, URL)
                .content
                .sentence
                .unwrap_or_default()
        };
        assert_eq!(sentence("admin"), "Ada invited you to join as an admin.");
        assert_eq!(
            sentence("billing"),
            "Ada invited you to join with billing access."
        );
    }

    #[test]
    fn only_alerts_can_be_turned_off() {
        assert!(EmailKind::BudgetAlert.is_alert());
        assert!(EmailKind::KeyExpiryWarning.is_alert());
        assert!(!EmailKind::PasswordChanged.is_alert());
        assert!(!EmailKind::MagicLink.is_alert());
        for email in all() {
            assert_eq!(
                email.kind.is_alert(),
                email.content.reason == Reason::Alert,
                "{}",
                email.kind.as_str()
            );
        }
    }

    #[test]
    fn german_and_chinese_render() {
        for lang in [Locale::De, Locale::ZhCn] {
            for email in all_in(lang) {
                let r = email.render(&instance());
                assert_eq!(r.lang, lang);
                insta::assert_snapshot!(
                    format!("{}_{}_text", email.kind.as_str(), lang.tag()),
                    format!("Subject: {}\n\n{}", r.subject, r.text)
                );
            }
        }
    }

    #[test]
    fn every_email_speaks_each_language() {
        let expected: [(EmailKind, [&str; 2], [&str; 2]); 18] = [
            (
                EmailKind::Invitation,
                [
                    "Ada Lovelace hat Sie zu Acme auf Kaeso eingeladen",
                    "Acme auf Kaeso beitreten",
                ],
                [
                    "Ada Lovelace 邀请您加入 Kaeso 上的 Acme",
                    "加入 Kaeso 上的 Acme",
                ],
            ),
            (
                EmailKind::InstanceInvitation,
                [
                    "Ada Lovelace hat Sie zu Kaeso eingeladen",
                    "Kaeso beitreten",
                ],
                ["Ada Lovelace 邀请您加入 Kaeso", "加入 Kaeso"],
            ),
            (
                EmailKind::MagicLink,
                ["Bei Kaeso anmelden", "Bei Kaeso anmelden"],
                ["登录 Kaeso", "登录 Kaeso"],
            ),
            (
                EmailKind::PasswordReset,
                ["Kaeso-Passwort zurücksetzen", "Passwort zurücksetzen"],
                ["重置您的 Kaeso 密码", "重置密码"],
            ),
            (
                EmailKind::EmailChangeConfirmation,
                [
                    "Neue E-Mail-Adresse für Kaeso bestätigen",
                    "Neue Adresse bestätigen",
                ],
                ["确认您的新 Kaeso 邮箱地址", "确认新地址"],
            ),
            (
                EmailKind::Signup,
                ["Kaeso-Konto fertig erstellen", "Kaeso-Konto erstellen"],
                ["完成创建您的 Kaeso 账户", "创建 Kaeso 账户"],
            ),
            (
                EmailKind::PasswordChanged,
                [
                    "Ihr Kaeso-Passwort wurde geändert",
                    "Ihr Passwort wurde geändert",
                ],
                ["您的 Kaeso 密码已更改", "密码已更改"],
            ),
            (
                EmailKind::TwoFactorEnabled,
                [
                    "Zwei-Faktor-Authentifizierung ist aktiviert",
                    "Zwei-Faktor-Authentifizierung ist aktiviert",
                ],
                ["双重验证已开启", "双重验证已开启"],
            ),
            (
                EmailKind::TwoFactorDisabled,
                [
                    "Zwei-Faktor-Authentifizierung ist deaktiviert",
                    "Zwei-Faktor-Authentifizierung ist deaktiviert",
                ],
                ["双重验证已关闭", "双重验证已关闭"],
            ),
            (
                EmailKind::TwoFactorReset,
                [
                    "Ihre Zwei-Faktor-Authentifizierung wurde zurückgesetzt",
                    "Zwei-Faktor-Authentifizierung zurückgesetzt",
                ],
                ["您的双重验证已被重置", "双重验证已重置"],
            ),
            (
                EmailKind::EmailChanged,
                [
                    "Ihre E-Mail-Adresse für Kaeso wurde geändert",
                    "Ihre E-Mail-Adresse wurde geändert",
                ],
                ["您的 Kaeso 邮箱地址已更改", "邮箱地址已更改"],
            ),
            (
                EmailKind::SessionsEnded,
                ["Sie wurden von Kaeso abgemeldet", "Sie wurden abgemeldet"],
                ["您已退出 Kaeso", "您已退出登录"],
            ),
            (
                EmailKind::AccountDeactivated,
                [
                    "Ihr Kaeso-Konto wurde deaktiviert",
                    "Ihr Konto wurde deaktiviert",
                ],
                ["您的 Kaeso 账户已停用", "账户已停用"],
            ),
            (
                EmailKind::OrgScheduledForDeletion,
                [
                    "Acme ist zur Löschung vorgemerkt",
                    "Acme wird am 10. Nov. 2026 gelöscht",
                ],
                ["Acme 已计划删除", "Acme 将于 2026年11月10日删除"],
            ),
            (
                EmailKind::OwnershipTransferred,
                [
                    "Sie sind jetzt Inhaber von Acme",
                    "Sie sind jetzt Inhaber von Acme",
                ],
                ["您现在是 Acme 的所有者", "您现在是 Acme 的所有者"],
            ),
            (
                EmailKind::KeyExpiryWarning,
                [
                    "Der API-Schlüssel ci-deploy läuft am 18. Okt. 2026 ab",
                    "API-Schlüssel ci-deploy läuft am 18. Okt. 2026 ab",
                ],
                [
                    "API 密钥 ci-deploy 将于 2026年10月18日过期",
                    "API 密钥 ci-deploy 将于 2026年10月18日过期",
                ],
            ),
            (
                EmailKind::BudgetAlert,
                [
                    "Das Projekt hat 80\u{a0}% des Monatsbudgets von 50 USD verbraucht",
                    "Ein Budget ist zu 80\u{a0}% verbraucht",
                ],
                ["该项目的每月支出预算（50 USD）已用 80%", "预算已用 80%"],
            ),
            (
                EmailKind::Test,
                ["Kaeso-Test-E-Mail", "E-Mail funktioniert"],
                ["Kaeso 测试邮件", "邮件发送正常"],
            ),
        ];
        for (lang, column) in [(Locale::De, 0), (Locale::ZhCn, 1)] {
            let emails = all_in(lang);
            assert_eq!(emails.len(), expected.len());
            for (email, (kind, de, zh)) in emails.iter().zip(&expected) {
                assert_eq!(email.kind, *kind);
                assert_eq!(email.lang, lang);
                let [subject, heading] = if column == 0 { de } else { zh };
                assert_eq!(&email.subject, subject, "{}", kind.as_str());
                assert_eq!(&email.content.heading, heading, "{}", kind.as_str());
            }
        }
    }

    fn skeleton(html: &str) -> Vec<String> {
        html.split('<')
            .skip(1)
            .map(|t| {
                t.split(|c: char| c.is_whitespace() || c == '>')
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect()
    }

    fn unspaced_seam(s: &str) -> Option<String> {
        let han = |c: char| ('\u{4e00}'..='\u{9fff}').contains(&c);
        let latin = |c: char| c.is_ascii_alphanumeric();
        let date = |c: char| matches!(c, '年' | '月' | '日');
        let chars: Vec<char> = s.chars().collect();
        chars.windows(2).find_map(|w| {
            let (a, b) = (w[0], w[1]);
            let bad = (han(a) && latin(b) && !(a.is_ascii_digit() || date(a)))
                || (latin(a) && han(b) && !(a.is_ascii_digit() && date(b)));
            bad.then(|| format!("{a}{b}"))
        })
    }

    #[test]
    fn translations_keep_the_layout_and_the_copy_rules() {
        let english = all();
        for lang in [Locale::De, Locale::ZhCn] {
            for (en, email) in english.iter().zip(all_in(lang)) {
                let kind = email.kind.as_str();
                let (r, r_en) = (email.render(&instance()), en.render(&instance()));
                let c = &email.content;
                assert_eq!(email.kind, en.kind);
                assert_eq!(c.reason, en.content.reason, "{kind}");
                assert_eq!(
                    c.sentence.is_some(),
                    en.content.sentence.is_some(),
                    "{kind}"
                );
                assert_eq!(c.note.is_some(), en.content.note.is_some(), "{kind}");
                assert_eq!(
                    c.action.as_ref().map(|a| &a.url),
                    en.content.action.as_ref().map(|a| &a.url),
                    "{kind}"
                );
                assert_eq!(skeleton(&r.html), skeleton(&r_en.html), "{kind}");
                let tag = lang.tag();
                assert!(r.html.contains(&format!(r#"<html lang="{tag}""#)), "{kind}");
                assert!(
                    r.html.contains(&format!(r#"lang="{tag}" style="#)),
                    "{kind}"
                );
                assert!(r.text.starts_with(&c.heading), "{kind}");
                assert!(!visible_text(&r.html).contains("://"), "{kind}");
                assert!(!r.text.contains("Sent by"), "{kind}");
                assert!(
                    email.subject.chars().count() <= 69,
                    "{kind}: {}",
                    email.subject
                );
                assert!(c.heading.chars().count() <= 55, "{kind}: {}", c.heading);
                let stop = if lang == Locale::ZhCn { "。" } else { "." };
                for line in [&c.sentence, &c.note].into_iter().flatten() {
                    assert!(line.chars().count() <= 115, "{kind}: {line}");
                    assert!(line.ends_with(stop), "{kind}: {line}");
                    let body = line.trim_end_matches(stop);
                    assert!(
                        !body.contains(". ") && !body.contains('。'),
                        "{kind}: {line}"
                    );
                }
                for text in [&email.subject, &c.heading] {
                    assert!(
                        !text.ends_with('.') && !text.ends_with('。'),
                        "{kind}: {text}"
                    );
                }
                if lang == Locale::ZhCn {
                    let all = [
                        Some(&email.subject),
                        Some(&c.heading),
                        c.sentence.as_ref(),
                        c.note.as_ref(),
                    ];
                    for text in all.into_iter().flatten() {
                        assert!(!text.contains([',', ';', ':', '?', '!']), "{kind}: {text}");
                        assert_eq!(unspaced_seam(text), None, "{kind}: {text}");
                    }
                    if let Some(a) = &c.action {
                        assert_eq!(unspaced_seam(a.label), None, "{kind}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_footer_speaks_the_language() {
        let footer = |lang, email: fn(Locale, &'static str) -> Email| {
            email(lang, TO)
                .render(&instance())
                .text
                .lines()
                .last()
                .unwrap()
                .to_owned()
        };
        assert_eq!(
            footer(Locale::De, password_changed),
            "Gesendet von Kaeso unter kaeso.example.com; Sicherheitshinweise werden immer gesendet."
        );
        assert_eq!(
            footer(Locale::ZhCn, password_changed),
            "此邮件由 kaeso.example.com 上的 Kaeso 发送；安全通知总会发送。"
        );
        assert_eq!(
            footer(Locale::ZhCn, test),
            "此邮件由 kaeso.example.com 上的 Kaeso 发送。"
        );
        let html = password_changed(Locale::ZhCn, TO).render(&instance()).html;
        assert!(html.contains(
            r#"此邮件由 <a href="https://kaeso.example.com/" style="color:#707070;text-decoration:none;">kaeso.example.com</a> 上的 Kaeso 发送；安全通知总会发送。"#
        ));
    }

    #[test]
    fn invitations_name_the_preset_in_each_language() {
        let sentence = |lang, preset| {
            invitation(lang, TO, "Ada", "Acme", preset, URL)
                .content
                .sentence
                .unwrap_or_default()
        };
        assert_eq!(
            sentence(Locale::De, "admin"),
            "Ada hat Sie eingeladen, als Admin beizutreten."
        );
        assert_eq!(
            sentence(Locale::De, "billing"),
            "Ada hat Sie eingeladen, mit Zugriff auf die Abrechnung beizutreten."
        );
        assert_eq!(
            sentence(Locale::ZhCn, "developer"),
            "Ada 邀请您以开发者身份加入。"
        );
        assert_eq!(
            sentence(Locale::ZhCn, "Data team"),
            "Ada 邀请您以 Data team 身份加入。"
        );
        assert_eq!(
            sentence(Locale::ZhCn, "数据组"),
            "Ada 邀请您以数据组身份加入。"
        );
    }

    #[test]
    fn who_did_it_is_named_in_each_language() {
        let sentence = |lang, by| {
            sessions_ended(lang, TO, by)
                .content
                .sentence
                .unwrap_or_default()
        };
        assert_eq!(
            sentence(Locale::En, Actor::InstallAdministrator),
            "An administrator of this Kaeso install ended every session of your account; sign in again to continue."
        );
        assert_eq!(
            sentence(Locale::De, Actor::InstallAdministrator),
            "Ein Admin dieser Kaeso-Instanz hat alle Sitzungen Ihres Kontos beendet; melden Sie sich erneut an."
        );
        assert_eq!(
            sentence(Locale::ZhCn, Actor::Operator),
            "此 Kaeso 实例的运营方结束了您账户的所有会话；请重新登录以继续。"
        );
        assert_eq!(
            sentence(Locale::ZhCn, Actor::Person("Ada")),
            "Ada 结束了您账户的所有会话；请重新登录以继续。"
        );
        for (actor, en) in [
            (Actor::Member, "A member"),
            (Actor::Owner, "An owner"),
            (Actor::OrgAdmin("Acme"), "An admin of Acme"),
            (Actor::Administrator, "An administrator"),
            (Actor::Operator, "The operator of this Kaeso install"),
        ] {
            assert_eq!(actor.name(Locale::En), en);
        }
        assert_eq!(Actor::OrgAdmin("Acme").name(Locale::ZhCn), "Acme 的管理员");
    }

    #[test]
    fn budgets_are_described_in_each_language() {
        let b = |kind, period, amount: &str| Budget {
            kind,
            period,
            amount: amount.to_owned(),
        };
        let tokens = b(LimitKind::TokenBudget, LimitPeriod::Day, "1500000");
        assert_eq!(tokens.english(), "daily token budget of 1500000 tokens");
        assert_eq!(
            tokens.german_genitive(),
            "Tagesbudgets von 1.500.000 Tokens"
        );
        assert_eq!(tokens.chinese(), "每日 Token 预算（1,500,000 个 Token）");
        let rate = b(LimitKind::RequestRate, LimitPeriod::Minute, "600");
        assert_eq!(rate.english(), "request rate of 600 requests per minute");
        assert_eq!(
            rate.german_genitive(),
            "Ratenlimits von 600 Anfragen pro Minute"
        );
        assert_eq!(rate.chinese(), "速率限制（每分钟 600 次请求）");
        let spend = b(LimitKind::SpendBudget, LimitPeriod::Total, "12.5");
        assert_eq!(spend.english(), "total spend budget of 12.5 USD");
        let alert = |lang| {
            budget_alert(lang, TO, &spend, BudgetScope::Key("key_01J9"), 90, URL)
                .content
                .sentence
                .unwrap_or_default()
        };
        assert_eq!(
            alert(Locale::En),
            "The API key key_01J9 has used 90% of its total spend budget of 12.5 USD."
        );
        assert_eq!(
            alert(Locale::De),
            "Der API-Schlüssel key_01J9 hat 90\u{a0}% des Gesamtbudgets von 12,5 USD verbraucht."
        );
        assert_eq!(
            alert(Locale::ZhCn),
            "API 密钥 key_01J9 的总支出预算（12.5 USD）已用 90%。"
        );
    }

    #[test]
    fn the_outlook_button_fits_each_language() {
        for lang in Locale::ALL {
            for email in all_in(lang) {
                if let Some(a) = &email.content.action {
                    let html = email.render(&instance()).html;
                    let min: usize = a
                        .label
                        .chars()
                        .map(|c| if c >= '\u{2e80}' { 15 } else { 9 })
                        .sum::<usize>()
                        + 44;
                    let at = html.find("v-text-anchor:middle;width:").unwrap() + 27;
                    let width: usize = html[at..].split("px").next().unwrap().parse().unwrap();
                    assert!(width >= min, "{}: {} < {min}", a.label, width);
                }
            }
        }
    }
}
