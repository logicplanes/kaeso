#![forbid(unsafe_code)]

pub mod accounts;
pub mod assembler;
pub mod catalogue;
pub mod clock;
pub mod cost;
pub mod decimal;
#[cfg(feature = "openapi")]
pub mod decimal_input;
pub mod email;
pub mod estimate;
pub mod id;
pub mod keys;
pub mod kpmr;
pub mod limits;
pub mod net;
pub mod provider;
pub mod providers;
pub mod rights;
pub mod secret;
pub mod text;
