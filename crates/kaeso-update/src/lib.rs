#![forbid(unsafe_code)]

pub mod manifest;
pub mod policy;
pub mod protocol;

pub use semver::Version;

pub const REPOSITORY: &str = "logicplanes/kaeso";
pub const RELEASE_WORKFLOW: &str =
    "https://github.com/logicplanes/kaeso/.github/workflows/release.yml@refs/heads/prod";
pub const DOWNLOADS: &str = "https://github.com/logicplanes/kaeso/releases/download/";
pub const UPGRADE_GUIDE: &str = "https://docs.kaeso.app/self-hosting/upgrades";
