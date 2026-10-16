use super::{OrgGrant, OrgRight, OrgRights, ProjectGrant, ProjectRight, ProjectRights, org_reach};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Owner,
    Role,
    Added,
    OrgRole,
    OrgRight(OrgRight),
}

pub fn org_sources(grant: &OrgGrant) -> Vec<(OrgRight, Vec<Source>)> {
    if grant.is_owner() {
        return OrgRights::all()
            .iter()
            .map(|r| (r, vec![Source::Owner]))
            .collect();
    }
    grant
        .effective()
        .iter()
        .map(|r| {
            let mut sources = Vec::new();
            if grant.base.contains(r) {
                sources.push(Source::Role);
            }
            if grant.added.contains(r) {
                sources.push(Source::Added);
            }
            (r, sources)
        })
        .collect()
}

pub fn every_project_sources(grant: &OrgGrant) -> Vec<(ProjectRight, Vec<Source>)> {
    let source = if grant.is_owner() {
        Source::Owner
    } else {
        Source::Role
    };
    grant
        .every_project()
        .iter()
        .map(|r| (r, vec![source]))
        .collect()
}

pub fn project_sources(
    org: Option<&OrgGrant>,
    project: Option<&ProjectGrant>,
) -> Vec<(ProjectRight, Vec<Source>)> {
    if org.is_some_and(OrgGrant::is_owner) {
        return ProjectRights::all()
            .iter()
            .map(|r| (r, vec![Source::Owner]))
            .collect();
    }
    let every = org.map_or(ProjectRights::EMPTY, OrgGrant::every_project);
    let org_rights = org.map_or(OrgRights::EMPTY, OrgGrant::effective);
    let reach = org_reach(org_rights);
    let effective = super::effective_project_rights(org, project);
    effective
        .iter()
        .map(|r| {
            let mut sources = Vec::new();
            if let Some(p) = project {
                if p.base.contains(r) && !p.removed.contains(r) {
                    sources.push(Source::Role);
                }
                if p.added.contains(r) && !p.removed.contains(r) {
                    sources.push(Source::Added);
                }
            }
            if every.contains(r) {
                sources.push(Source::OrgRole);
            }
            if reach.contains(r) {
                for via in [OrgRight::ProjectsViewAll, OrgRight::ProjectsArchiveAny] {
                    if org_rights.contains(via) && org_reach(OrgRights::EMPTY.with(via)).contains(r)
                    {
                        sources.push(Source::OrgRight(via));
                    }
                }
            }
            (r, sources)
        })
        .collect()
}
