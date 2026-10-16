#![forbid(unsafe_code)]

pub mod assets;
mod layout;
mod locale;
mod mailer;
mod style;
pub mod templates;

pub use layout::Instance;
pub use locale::Locale;
pub use mailer::{MailError, Mailer, MemorySink, SmtpSecurity, SmtpSettings, Transport};
pub use templates::{
    Actor, Budget, BudgetScope, Email, EmailKind, LimitKind, LimitPeriod, Rendered,
};
