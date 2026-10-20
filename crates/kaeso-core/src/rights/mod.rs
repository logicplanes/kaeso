mod catalogue;
mod descriptions;
mod presets;
mod sources;

pub use catalogue::{Category, OrgRight, ProjectRight, Right, RightParseError};
pub use presets::{OrgPreset, PresetParseError, ProjectPreset};
pub use sources::{Source, every_project_sources, org_sources, project_sources};

use crate::id::RoleId;

use std::fmt;

macro_rules! right_set {
    ($(#[$doc:meta])* $name:ident, $right:ty) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
        pub struct $name(u64);

        impl $name {
            pub const EMPTY: $name = $name(0);

            pub fn all() -> $name {
                <$right>::ALL.iter().copied().collect()
            }

            pub fn contains(self, right: $right) -> bool {
                self.0 & right.bit() != 0
            }

            pub fn insert(&mut self, right: $right) {
                self.0 |= right.bit();
            }

            #[must_use]
            pub fn with(mut self, right: $right) -> $name {
                self.insert(right);
                self
            }

            #[must_use]
            pub fn union(self, other: $name) -> $name {
                $name(self.0 | other.0)
            }

            #[must_use]
            pub fn minus(self, other: $name) -> $name {
                $name(self.0 & !other.0)
            }

            #[must_use]
            pub fn intersect(self, other: $name) -> $name {
                $name(self.0 & other.0)
            }

            pub fn is_subset(self, other: $name) -> bool {
                self.0 & !other.0 == 0
            }

            pub fn is_empty(self) -> bool {
                self.0 == 0
            }

            pub fn iter(self) -> impl Iterator<Item = $right> {
                <$right>::ALL.iter().copied().filter(move |r| self.contains(*r))
            }

            pub fn names(self) -> Vec<&'static str> {
                self.iter().map(<$right>::as_str).collect()
            }

            pub fn parse<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<$name, RightParseError> {
                names.into_iter().map(str::parse::<$right>).collect()
            }
        }

        impl FromIterator<$right> for $name {
            fn from_iter<I: IntoIterator<Item = $right>>(iter: I) -> $name {
                let mut set = $name::EMPTY;
                for r in iter {
                    set.insert(r);
                }
                set
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_set().entries(self.names()).finish()
            }
        }
    };
}

right_set!(OrgRights, OrgRight);
right_set!(ProjectRights, ProjectRight);

impl OrgRights {
    pub fn owner_only() -> OrgRights {
        OrgRight::ALL
            .iter()
            .copied()
            .filter(|r| r.owner_only())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrgRole {
    Builtin(OrgPreset),
    Custom(RoleId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProjectRole {
    Builtin(ProjectPreset),
    Custom(RoleId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrgGrant {
    pub role: OrgRole,
    pub base: OrgRights,
    pub every_project: ProjectRights,
    pub added: OrgRights,
    pub removed: OrgRights,
}

impl OrgGrant {
    pub fn preset(preset: OrgPreset) -> OrgGrant {
        OrgGrant {
            role: OrgRole::Builtin(preset),
            base: preset.rights(),
            every_project: preset.project_rights(),
            added: OrgRights::EMPTY,
            removed: OrgRights::EMPTY,
        }
    }

    pub fn custom(id: RoleId, rights: OrgRights, every_project: ProjectRights) -> OrgGrant {
        OrgGrant {
            role: OrgRole::Custom(id),
            base: rights,
            every_project,
            added: OrgRights::EMPTY,
            removed: OrgRights::EMPTY,
        }
    }

    pub fn preset_role(&self) -> Option<OrgPreset> {
        match self.role {
            OrgRole::Builtin(p) => Some(p),
            OrgRole::Custom(_) => None,
        }
    }

    pub fn is_owner(&self) -> bool {
        self.role == OrgRole::Builtin(OrgPreset::Owner)
    }

    pub fn effective(&self) -> OrgRights {
        if self.is_owner() {
            return OrgRights::all();
        }
        self.base
            .union(self.added)
            .minus(self.removed)
            .minus(OrgRights::owner_only())
    }

    pub fn every_project(&self) -> ProjectRights {
        if self.is_owner() {
            return ProjectRights::all();
        }
        self.every_project
    }

    pub fn default_project_preset(&self) -> ProjectPreset {
        self.preset_role()
            .map_or(ProjectPreset::Viewer, OrgPreset::default_project_preset)
    }

    #[must_use]
    pub fn normalised(self) -> OrgGrant {
        if self.is_owner() {
            return OrgGrant::preset(OrgPreset::Owner);
        }
        OrgGrant {
            added: self
                .added
                .minus(self.base)
                .minus(self.removed)
                .minus(OrgRights::owner_only()),
            removed: self.removed.intersect(self.base),
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectGrant {
    pub role: ProjectRole,
    pub base: ProjectRights,
    pub added: ProjectRights,
    pub removed: ProjectRights,
}

impl ProjectGrant {
    pub fn preset(preset: ProjectPreset) -> ProjectGrant {
        ProjectGrant {
            role: ProjectRole::Builtin(preset),
            base: preset.rights(),
            added: ProjectRights::EMPTY,
            removed: ProjectRights::EMPTY,
        }
    }

    pub fn custom(id: RoleId, rights: ProjectRights) -> ProjectGrant {
        ProjectGrant {
            role: ProjectRole::Custom(id),
            base: rights,
            added: ProjectRights::EMPTY,
            removed: ProjectRights::EMPTY,
        }
    }

    pub fn preset_role(&self) -> Option<ProjectPreset> {
        match self.role {
            ProjectRole::Builtin(p) => Some(p),
            ProjectRole::Custom(_) => None,
        }
    }

    pub fn effective(&self) -> ProjectRights {
        self.base.union(self.added).minus(self.removed)
    }

    #[must_use]
    pub fn normalised(self) -> ProjectGrant {
        ProjectGrant {
            added: self.added.minus(self.base).minus(self.removed),
            removed: self.removed.intersect(self.base),
            ..self
        }
    }
}

pub fn effective_project_rights(
    org: Option<&OrgGrant>,
    project: Option<&ProjectGrant>,
) -> ProjectRights {
    if org.is_some_and(OrgGrant::is_owner) {
        return ProjectRights::all();
    }
    let own = project.map_or(ProjectRights::EMPTY, ProjectGrant::effective);
    org.map_or(own, |o| {
        own.union(o.every_project()).union(org_reach(o.effective()))
    })
}

pub fn org_reach(org: OrgRights) -> ProjectRights {
    let mut reach = ProjectRights::EMPTY;
    if org.contains(OrgRight::ProjectsViewAll) {
        reach.insert(ProjectRight::ProjectView);
    }
    if org.contains(OrgRight::ProjectsArchiveAny) {
        reach.insert(ProjectRight::ProjectView);
        reach.insert(ProjectRight::ProjectArchive);
    }
    reach
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("You can't grant {}: you don't hold {} yourself.", listed(.missing), if .missing.len() == 1 { "it" } else { "them" })]
pub struct GrantExceedsHolder {
    pub missing: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MissingDependency {
    #[error("`{right}` needs `{needs}`.")]
    Gained {
        right: &'static str,
        needs: &'static str,
    },
    #[error("`{right}` needs `{needs}`; remove `{right}` too.")]
    Removed {
        right: &'static str,
        needs: &'static str,
    },
}

fn listed(names: &[&'static str]) -> String {
    let first = names.first().copied().unwrap_or_default();
    match names.len() {
        0 | 1 => format!("`{first}`"),
        2 => format!("`{first}` and 1 more right"),
        n => format!("`{first}` and {} more rights", n - 1),
    }
}

pub fn check_org_grant(
    grantor: OrgRights,
    before: OrgRights,
    after: OrgRights,
) -> Result<(), GrantExceedsHolder> {
    let missing = after.minus(before).minus(grantor);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(GrantExceedsHolder {
            missing: missing.names(),
        })
    }
}

pub fn check_project_grant(
    grantor: ProjectRights,
    before: ProjectRights,
    after: ProjectRights,
) -> Result<(), GrantExceedsHolder> {
    let missing = after.minus(before).minus(grantor);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(GrantExceedsHolder {
            missing: missing.names(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StrandedHolders {
    #[error("`{right}` needs `{needs}` for {count} of this role's holders; keep it in the role.")]
    Removed {
        right: &'static str,
        needs: &'static str,
        count: usize,
    },
    #[error("`{right}` needs `{needs}`, removed from {count} of this role's holders; restore it.")]
    Gained {
        right: &'static str,
        needs: &'static str,
        count: usize,
    },
}

impl StrandedHolders {
    fn of(missing: MissingDependency, count: usize) -> StrandedHolders {
        match missing {
            MissingDependency::Removed { right, needs } => StrandedHolders::Removed {
                right,
                needs,
                count,
            },
            MissingDependency::Gained { right, needs } => StrandedHolders::Gained {
                right,
                needs,
                count,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Deviation<S> {
    pub added: S,
    pub removed: S,
}

macro_rules! check_needs {
    ($name:ident, $missing:ident, $holders:ident, $set:ty) => {
        fn $missing(before: $set, after: $set) -> impl Iterator<Item = MissingDependency> {
            after.iter().flat_map(move |right| {
                right
                    .needs()
                    .into_iter()
                    .filter(move |n| !after.contains(*n))
                    .filter_map(move |needs| {
                        if !before.contains(right) {
                            Some(MissingDependency::Gained {
                                right: right.as_str(),
                                needs: needs.as_str(),
                            })
                        } else if before.contains(needs) {
                            Some(MissingDependency::Removed {
                                right: right.as_str(),
                                needs: needs.as_str(),
                            })
                        } else {
                            None
                        }
                    })
            })
        }

        pub fn $name(before: $set, after: $set) -> Result<(), MissingDependency> {
            $missing(before, after).next().map_or(Ok(()), Err)
        }

        pub fn $holders(
            before: $set,
            after: $set,
            holders: &[Deviation<$set>],
        ) -> Result<(), StrandedHolders> {
            let held = |role: $set, d: &Deviation<$set>| role.union(d.added).minus(d.removed);
            let missing = |d: &Deviation<$set>| $missing(held(before, d), held(after, d));
            let Some(first) = holders.iter().find_map(|d| missing(d).next()) else {
                return Ok(());
            };
            let count = holders
                .iter()
                .filter(|d| missing(d).any(|m| m == first))
                .count();
            Err(StrandedHolders::of(first, count))
        }
    };
}

check_needs!(
    check_org_needs,
    org_missing,
    check_org_role_holders,
    OrgRights
);
check_needs!(
    check_project_needs,
    project_missing,
    check_project_role_holders,
    ProjectRights
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct OrgHolding {
    pub org: OrgRights,
    pub every_project: ProjectRights,
}

impl OrgHolding {
    pub fn of(grant: &OrgGrant) -> OrgHolding {
        OrgHolding {
            org: grant.effective(),
            every_project: grant.every_project(),
        }
    }

    pub fn covers(&self, other: &OrgHolding) -> bool {
        other.org.is_subset(self.org) && other.every_project.is_subset(self.every_project)
    }

    pub fn check_change(
        &self,
        before: &OrgHolding,
        after: &OrgHolding,
    ) -> Result<(), GrantCheckError> {
        check_org_grant(self.org, before.org, after.org)?;
        check_project_grant(
            self.every_project,
            before.every_project,
            after.every_project,
        )?;
        check_org_needs(before.org, after.org)?;
        check_project_needs(before.every_project, after.every_project)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GrantCheckError {
    #[error(transparent)]
    Exceeds(#[from] GrantExceedsHolder),
    #[error(transparent)]
    Dependency(#[from] MissingDependency),
}

pub fn check_project_change(
    grantor: ProjectRights,
    before: ProjectRights,
    after: ProjectRights,
) -> Result<(), GrantCheckError> {
    check_project_grant(grantor, before, after)?;
    check_project_needs(before, after)?;
    Ok(())
}

#[cfg(test)]
mod tests;
