// The updater's compose runner checks an install's gateway with these rules before it switches,
// because an update that migrates the databases can't roll back once the new gateway has started.

use std::time::Duration;

use crate::duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    pub key: &'static str,
    pub env: &'static str,
}

impl Setting {
    pub fn table(self) -> &'static str {
        self.key.split_once('.').map_or("", |(table, _)| table)
    }

    pub fn field(self) -> &'static str {
        self.key
            .split_once('.')
            .map_or(self.key, |(_, field)| field)
    }

    fn named(self) -> String {
        format!("{} ({})", self.env, self.key)
    }
}

pub const KPMR_INTERVAL: Setting = Setting {
    key: "kpmr.interval",
    env: "KAESO_KPMR_INTERVAL",
};
pub const MCP_SYNC_INTERVAL: Setting = Setting {
    key: "mcp_servers.sync_interval",
    env: "KAESO_MCP_SERVERS_SYNC_INTERVAL",
};
pub const SMTP_USER: Setting = Setting {
    key: "smtp.user",
    env: "KAESO_SMTP_USER",
};
pub const SMTP_PASSWORD: Setting = Setting {
    key: "smtp.password",
    env: "KAESO_SMTP_PASSWORD",
};
pub const CHECKED: [Setting; 4] = [KPMR_INTERVAL, MCP_SYNC_INTERVAL, SMTP_USER, SMTP_PASSWORD];

// The kpmr_sync job runs every 5 minutes, and mcp_sync asks no server more often than once a
// minute, so a shorter interval would be raised without a word.
pub const KPMR_SHORTEST: Duration = Duration::from_secs(300);
pub const MCP_SHORTEST: Duration = Duration::from_secs(60);

pub fn kpmr_interval_too_short(found: Duration) -> String {
    format!(
        "is {}, and it must be `auto` or at least 5m: set {}=5m or longer, or `{} = \"5m\"` under `[{}]` in the configuration file, or remove it for `auto`",
        duration::shown(found),
        KPMR_INTERVAL.env,
        KPMR_INTERVAL.field(),
        KPMR_INTERVAL.table(),
    )
}

pub fn mcp_sync_interval_too_short(found: Duration) -> String {
    format!(
        "is {}, and it must be at least 1m: set {}=1m or longer, or `{} = \"1m\"` under `[{}]` in the configuration file, or remove it for the default",
        duration::shown(found),
        MCP_SYNC_INTERVAL.env,
        MCP_SYNC_INTERVAL.field(),
        MCP_SYNC_INTERVAL.table(),
    )
}

// Without a password the mailer would send without authentication, and the first email would
// fail much later. The user name stays out: some providers' SMTP user is an access key id.
pub fn smtp_user_alone() -> String {
    format!(
        "is required when {} is set: set {}, or `{}` under `[{}]` in the configuration file; for a server without authentication, remove {}, or `{}` under `[{}]`",
        SMTP_USER.key,
        SMTP_PASSWORD.env,
        SMTP_PASSWORD.field(),
        SMTP_PASSWORD.table(),
        SMTP_USER.env,
        SMTP_USER.field(),
        SMTP_USER.table(),
    )
}

// A value that does not parse is left out: the running gateway would have refused it already.
pub fn refused(value: impl Fn(Setting) -> Option<String>) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(text) = value(KPMR_INTERVAL)
        && text.trim() != "auto"
        && let Ok(every) = duration::parse(&text)
        && every < KPMR_SHORTEST
    {
        found.push(format!(
            "{} {}",
            KPMR_INTERVAL.named(),
            kpmr_interval_too_short(every)
        ));
    }
    if let Some(Ok(every)) = value(MCP_SYNC_INTERVAL).map(|t| duration::parse(&t))
        && every < MCP_SHORTEST
    {
        found.push(format!(
            "{} {}",
            MCP_SYNC_INTERVAL.named(),
            mcp_sync_interval_too_short(every)
        ));
    }
    if value(SMTP_USER).is_some() && value(SMTP_PASSWORD).is_none() {
        found.push(format!("{} {}", SMTP_PASSWORD.named(), smtp_user_alone()));
    }
    found
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn refused_with(values: &[(Setting, &str)]) -> Vec<String> {
        let values: HashMap<&str, &str> = values.iter().map(|(s, v)| (s.key, *v)).collect();
        refused(|s| values.get(s.key).map(|v| (*v).to_owned()))
    }

    #[test]
    fn each_refusal_names_the_setting_the_value_and_the_fix_in_both_forms() {
        assert_eq!(
            refused_with(&[(KPMR_INTERVAL, "1m")]),
            [
                "KAESO_KPMR_INTERVAL (kpmr.interval) is 1m, and it must be `auto` or at least 5m: set KAESO_KPMR_INTERVAL=5m or longer, or `interval = \"5m\"` under `[kpmr]` in the configuration file, or remove it for `auto`"
            ]
        );
        assert_eq!(
            refused_with(&[(MCP_SYNC_INTERVAL, "30s")]),
            [
                "KAESO_MCP_SERVERS_SYNC_INTERVAL (mcp_servers.sync_interval) is 30s, and it must be at least 1m: set KAESO_MCP_SERVERS_SYNC_INTERVAL=1m or longer, or `sync_interval = \"1m\"` under `[mcp_servers]` in the configuration file, or remove it for the default"
            ]
        );
        assert_eq!(
            refused_with(&[(SMTP_USER, "AKIAEXAMPLE")]),
            [
                "KAESO_SMTP_PASSWORD (smtp.password) is required when smtp.user is set: set KAESO_SMTP_PASSWORD, or `password` under `[smtp]` in the configuration file; for a server without authentication, remove KAESO_SMTP_USER, or `user` under `[smtp]`"
            ]
        );
    }

    #[test]
    fn the_shortest_intervals_and_auto_pass() {
        for values in [
            &[(KPMR_INTERVAL, "5m")][..],
            &[(KPMR_INTERVAL, "300s")],
            &[(KPMR_INTERVAL, " auto ")],
            &[(MCP_SYNC_INTERVAL, "1m")],
            &[(MCP_SYNC_INTERVAL, "60s")],
            &[(SMTP_USER, "kaeso"), (SMTP_PASSWORD, "pw")],
            &[(SMTP_PASSWORD, "pw")],
            &[],
        ] {
            assert!(refused_with(values).is_empty(), "{values:?}");
        }
    }

    #[test]
    fn values_just_under_the_minimum_and_zero_are_refused() {
        for (setting, value, shown) in [
            (KPMR_INTERVAL, "299s", "4m 59s"),
            (KPMR_INTERVAL, "0", "0s"),
            (MCP_SYNC_INTERVAL, "59s", "59s"),
            (MCP_SYNC_INTERVAL, "0s", "0s"),
        ] {
            let found = refused_with(&[(setting, value)]);
            assert_eq!(found.len(), 1, "{value}");
            assert!(
                found[0].starts_with(&format!("{} ({}) is {shown}, ", setting.env, setting.key)),
                "{found:?}"
            );
        }
    }

    #[test]
    fn values_that_do_not_parse_are_left_to_the_gateway() {
        assert!(refused_with(&[(KPMR_INTERVAL, "soon"), (MCP_SYNC_INTERVAL, "5")]).is_empty());
    }

    #[test]
    fn every_refused_setting_is_listed_once() {
        let found = refused_with(&[
            (KPMR_INTERVAL, "2m"),
            (MCP_SYNC_INTERVAL, "10s"),
            (SMTP_USER, "kaeso"),
        ]);
        let named: Vec<&str> = found.iter().map(|r| r.split(' ').next().unwrap()).collect();
        assert_eq!(
            named,
            [
                "KAESO_KPMR_INTERVAL",
                "KAESO_MCP_SERVERS_SYNC_INTERVAL",
                "KAESO_SMTP_PASSWORD"
            ]
        );
    }
}
