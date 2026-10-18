use semver::Version;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Stable,
    Prerelease,
}

impl Channel {
    pub const ALL: [Channel; 2] = [Channel::Stable, Channel::Prerelease];

    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Prerelease => "prerelease",
        }
    }

    pub fn parse(s: &str) -> Option<Channel> {
        Channel::ALL.into_iter().find(|c| c.as_str() == s)
    }

    pub fn offers(self, version: &Version) -> bool {
        self == Channel::Prerelease || version.pre.is_empty()
    }
}

pub fn parse_tag(tag: &str) -> Option<Version> {
    let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
    version.build.is_empty().then_some(version)
}

pub fn is_major(from: &Version, to: &Version) -> bool {
    to.major > from.major
}

pub trait Release {
    fn version(&self) -> &Version;
    fn needs_manual_steps(&self) -> bool;
    fn prepared(&self) -> bool;
    fn asks_for_steps_after(&self, _running: &Version) -> bool {
        false
    }
    fn prerelease(&self) -> bool {
        !self.version().pre.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Manual {
    Major,
    ManualSteps,
    NotPrepared,
}

impl Manual {
    pub fn as_str(self) -> &'static str {
        match self {
            Manual::Major => "major",
            Manual::ManualSteps => "manual_steps",
            Manual::NotPrepared => "not_prepared",
        }
    }
}

#[derive(Debug)]
pub struct Plan<'a, R> {
    pub target: &'a R,
    pub newer: Vec<&'a R>,
    pub manual: Option<Manual>,
}

pub fn plan<'a, R: Release>(
    running: &Version,
    channel: Channel,
    releases: &'a [R],
) -> Option<Plan<'a, R>> {
    let mut newer: Vec<&R> = releases
        .iter()
        .filter(|r| r.version() > running && (channel == Channel::Prerelease || !r.prerelease()))
        .collect();
    newer.sort_by(|a, b| b.version().cmp(a.version()));
    let target = *newer.first()?;
    let crossed = |r: &&R| r.version() > running && r.version() <= target.version();
    let manual = if is_major(running, target.version()) {
        Some(Manual::Major)
    } else if target.asks_for_steps_after(running)
        || releases
            .iter()
            .filter(crossed)
            .any(Release::needs_manual_steps)
    {
        Some(Manual::ManualSteps)
    } else if !target.prepared() {
        Some(Manual::NotPrepared)
    } else {
        None
    };
    Some(Plan {
        target,
        newer,
        manual,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[derive(Debug)]
    struct R {
        version: Version,
        manual: bool,
        prepared: bool,
        manual_since: Option<Version>,
    }

    impl Release for R {
        fn version(&self) -> &Version {
            &self.version
        }
        fn needs_manual_steps(&self) -> bool {
            self.manual
        }
        fn prepared(&self) -> bool {
            self.prepared
        }
        fn asks_for_steps_after(&self, running: &Version) -> bool {
            self.manual_since.as_ref().is_some_and(|s| s > running)
        }
    }

    fn r(v: &str) -> R {
        R {
            version: Version::parse(v).unwrap(),
            manual: false,
            prepared: true,
            manual_since: None,
        }
    }

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    fn target(running: &str, channel: Channel, releases: &[R]) -> Option<(String, Option<Manual>)> {
        plan(&v(running), channel, releases).map(|p| (p.target.version.to_string(), p.manual))
    }

    #[test]
    fn tags_are_versions_with_or_without_v_and_never_with_build_metadata() {
        assert_eq!(parse_tag("v41.4.0"), Some(v("41.4.0")));
        assert_eq!(parse_tag("41.4.0-rc.1"), Some(v("41.4.0-rc.1")));
        assert_eq!(parse_tag("v1.4"), None);
        assert_eq!(parse_tag("v41.4.0+build.7"), None);
        assert_eq!(parse_tag("latest"), None);
    }

    #[test]
    fn channels_parse_their_names_and_stable_never_offers_pre_releases() {
        for c in Channel::ALL {
            assert_eq!(Channel::parse(c.as_str()), Some(c));
        }
        assert_eq!(Channel::parse("beta"), None);
        assert!(!Channel::Stable.offers(&v("40.3.0-rc.1")));
        assert!(Channel::Stable.offers(&v("40.3.0")));
        assert!(Channel::Prerelease.offers(&v("40.3.0-rc.1")));
    }

    #[test]
    fn the_newest_offered_release_after_the_running_one_is_the_target() {
        let releases = [r("40.1.9"), r("40.2.0"), r("40.2.1"), r("40.3.0-rc.1")];
        assert_eq!(
            target("40.2.0", Channel::Stable, &releases),
            Some(("40.2.1".into(), None))
        );
        assert_eq!(
            target("40.2.0", Channel::Prerelease, &releases),
            Some(("40.3.0-rc.1".into(), None))
        );
        assert_eq!(target("40.2.1", Channel::Stable, &releases), None);
        assert_eq!(target("40.4.0", Channel::Prerelease, &releases), None);
    }

    struct Flagged(R);

    impl Release for Flagged {
        fn version(&self) -> &Version {
            &self.0.version
        }
        fn needs_manual_steps(&self) -> bool {
            false
        }
        fn prepared(&self) -> bool {
            true
        }
        fn prerelease(&self) -> bool {
            true
        }
    }

    #[test]
    fn a_release_published_as_a_pre_release_is_one_whatever_its_version() {
        let releases = [Flagged(r("40.2.1"))];
        assert!(plan(&v("40.2.0"), Channel::Stable, &releases).is_none());
        assert_eq!(
            plan(&v("40.2.0"), Channel::Prerelease, &releases).map(|p| p
                .target
                .0
                .version
                .to_string()),
            Some("40.2.1".to_owned())
        );
    }

    #[test]
    fn a_running_pre_release_is_offered_its_stable_release() {
        let releases = [r("40.3.0-rc.1"), r("40.3.0")];
        assert_eq!(
            target("40.3.0-rc.1", Channel::Stable, &releases),
            Some(("40.3.0".into(), None))
        );
    }

    #[test]
    fn a_new_major_version_is_always_manual() {
        let releases = [r("41.0.0"), r("40.9.0")];
        assert_eq!(
            target("40.8.0", Channel::Stable, &releases),
            Some(("41.0.0".into(), Some(Manual::Major)))
        );
    }

    #[test]
    fn manual_steps_of_any_crossed_release_make_the_update_manual() {
        let mut middle = r("40.3.0");
        middle.manual = true;
        let releases = [r("40.2.0"), middle, r("40.4.0")];
        assert_eq!(
            target("40.2.0", Channel::Stable, &releases),
            Some(("40.4.0".into(), Some(Manual::ManualSteps)))
        );
        assert_eq!(
            target("40.3.0", Channel::Stable, &releases),
            Some(("40.4.0".into(), None))
        );
    }

    #[test]
    fn manual_steps_of_a_skipped_pre_release_still_count() {
        let mut rc = r("40.3.0-rc.1");
        rc.manual = true;
        let releases = [rc, r("40.3.0")];
        assert_eq!(
            target("40.2.0", Channel::Stable, &releases),
            Some(("40.3.0".into(), Some(Manual::ManualSteps)))
        );
    }

    #[test]
    fn a_release_that_names_steps_by_hand_since_the_running_version_is_manual() {
        let mut target = r("40.4.0");
        target.manual_since = Some(v("40.3.0"));
        let releases = [target];
        assert_eq!(
            target_of("40.2.0", &releases),
            Some(("40.4.0".into(), Some(Manual::ManualSteps))),
            "40.3.0 asked for steps by hand though the gateway never saw it"
        );
        assert_eq!(
            target_of("40.3.0", &releases),
            Some(("40.4.0".into(), None))
        );
    }

    fn target_of(running: &str, releases: &[R]) -> Option<(String, Option<Manual>)> {
        target(running, Channel::Stable, releases)
    }

    #[test]
    fn a_release_without_a_manifest_cannot_be_applied_in_one_click() {
        let mut old = r("40.2.0");
        old.prepared = false;
        assert_eq!(
            target("40.1.9", Channel::Stable, &[old]),
            Some(("40.2.0".into(), Some(Manual::NotPrepared)))
        );
    }

    #[test]
    fn newer_lists_every_offered_release_newest_first() {
        let releases = [r("40.2.0"), r("40.4.0"), r("40.3.0"), r("40.5.0-rc.1")];
        let p = plan(&v("40.2.0"), Channel::Stable, &releases).unwrap();
        let newer: Vec<String> = p.newer.iter().map(|r| r.version.to_string()).collect();
        assert_eq!(newer, ["40.4.0", "40.3.0"]);
    }

    #[test]
    fn manual_reasons_have_stable_names() {
        assert_eq!(Manual::Major.as_str(), "major");
        assert_eq!(Manual::ManualSteps.as_str(), "manual_steps");
        assert_eq!(Manual::NotPrepared.as_str(), "not_prepared");
    }

    proptest! {
        #[test]
        fn the_target_is_never_older_than_the_running_version_and_never_a_pre_release_on_stable(
            running in (0u64..3, 0u64..5, 0u64..5),
            versions in proptest::collection::vec((0u64..3, 0u64..5, 0u64..5, proptest::option::of(1u64..3)), 0..12),
        ) {
            let running = Version::new(running.0, running.1, running.2);
            let releases: Vec<R> = versions
                .iter()
                .map(|(a, b, c, pre)| {
                    let text = match pre {
                        Some(n) => format!("{a}.{b}.{c}-rc.{n}"),
                        None => format!("{a}.{b}.{c}"),
                    };
                    r(&text)
                })
                .collect();
            if let Some(p) = plan(&running, Channel::Stable, &releases) {
                prop_assert!(p.target.version > running);
                prop_assert!(p.target.version.pre.is_empty());
                prop_assert!(p.newer.iter().all(|n| n.version <= p.target.version));
                prop_assert_eq!(p.manual == Some(Manual::Major), p.target.version.major > running.major);
            }
        }
    }
}
