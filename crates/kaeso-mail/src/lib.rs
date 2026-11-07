#![forbid(unsafe_code)]

pub mod assets;
mod layout;
mod locale;
mod mailer;
pub mod notices;
mod right_names;
mod style;
pub mod templates;

pub use layout::Instance;
pub use locale::Locale;
pub use mailer::{MailError, Mailer, MemorySink, SmtpSecurity, SmtpSettings, Transport};
pub use templates::{
    Actor, Budget, BudgetAlert, BudgetScope, Email, EmailKind, LimitKind, LimitPeriod, Rendered,
};
