use proptest::prelude::*;

use super::*;

#[test]
fn categories_are_contiguous_and_rights_are_sentences() {
    fn contiguous(cats: Vec<Category>) {
        let mut seen: Vec<Category> = Vec::new();
        for c in cats {
            if seen.last() != Some(&c) {
                assert!(!seen.contains(&c), "{c:?} appears twice");
                seen.push(c);
            }
        }
    }
    contiguous(OrgRight::ALL.iter().map(|r| r.category()).collect());
    contiguous(ProjectRight::ALL.iter().map(|r| r.category()).collect());
    for d in OrgRight::ALL
        .iter()
        .map(|r| r.description())
        .chain(ProjectRight::ALL.iter().map(|r| r.description()))
    {
        assert!(d.ends_with('.') && d.len() <= 120, "{d}");
    }
}

#[test]
fn needs_point_to_the_same_scope_and_have_no_cycles() {
    for r in OrgRight::ALL {
        assert!(!r.needs().contains(r), "{r} needs itself");
        for n in r.needs() {
            assert!(!n.needs().contains(r), "{r} and {n} need each other");
        }
    }
    for r in ProjectRight::ALL {
        assert!(!r.needs().contains(r), "{r} needs itself");
    }
    assert!(OrgRight::OrgView.needs().is_empty());
    assert_eq!(
        ProjectRight::ModelsManage.needs(),
        [ProjectRight::ProjectView, ProjectRight::ModelsView]
    );
}

#[test]
fn every_built_in_role_satisfies_the_needs() {
    for p in OrgPreset::ALL {
        let rights = p.rights();
        assert!(check_org_needs(OrgRights::EMPTY, rights).is_ok(), "{p}");
        let every = p.project_rights();
        assert!(
            check_project_needs(ProjectRights::EMPTY, every).is_ok(),
            "{p} in every project"
        );
    }
    for p in ProjectPreset::ALL {
        let rights = p.rights();
        assert!(
            check_project_needs(ProjectRights::EMPTY, rights).is_ok(),
            "{p}"
        );
    }
}

#[test]
fn owners_and_admins_hold_every_project_right() {
    for preset in OrgPreset::ALL {
        let grant = OrgGrant::preset(preset);
        let expected = matches!(preset, OrgPreset::Owner | OrgPreset::Admin);
        assert_eq!(
            effective_project_rights(Some(&grant), None) == ProjectRights::all(),
            expected,
            "{preset}"
        );
    }
    let dev = ProjectGrant::preset(ProjectPreset::Developer);
    assert_eq!(
        effective_project_rights(Some(&OrgGrant::preset(OrgPreset::Member)), Some(&dev)),
        ProjectPreset::Developer.rights()
    );
    assert!(effective_project_rights(None, None).is_empty());
    let billing = OrgGrant::preset(OrgPreset::Billing);
    assert_eq!(
        effective_project_rights(Some(&billing), None).names(),
        ["project.view"]
    );
    let archiver = OrgGrant {
        added: [OrgRight::ProjectsArchiveAny].into_iter().collect(),
        ..OrgGrant::preset(OrgPreset::Developer)
    };
    assert_eq!(
        effective_project_rights(Some(&archiver), None).names(),
        ["project.view", "project.archive"]
    );
    assert!(effective_project_rights(Some(&OrgGrant::preset(OrgPreset::Member)), None).is_empty());
    let auditor = effective_project_rights(Some(&OrgGrant::preset(OrgPreset::Auditor)), None);
    assert!(auditor.contains(ProjectRight::ProjectAuditLogView));
    assert!(auditor.contains(ProjectRight::LogsMetadataView));
    assert!(!auditor.contains(ProjectRight::LogsContentView));
    let id = crate::id::RoleId::new();
    let custom = OrgGrant::custom(
        id,
        OrgPreset::Member.rights(),
        ProjectRights::EMPTY
            .with(ProjectRight::ProjectView)
            .with(ProjectRight::UsageView),
    );
    assert_eq!(
        effective_project_rights(Some(&custom), None).names(),
        ["project.view", "usage.view"]
    );
    assert_eq!(custom.default_project_preset(), ProjectPreset::Viewer);
}

#[test]
fn sources_name_every_reason() {
    let org = OrgGrant {
        added: [OrgRight::ProjectsViewAll, OrgRight::LogsDelete]
            .into_iter()
            .collect(),
        removed: [OrgRight::MembersView].into_iter().collect(),
        ..OrgGrant::preset(OrgPreset::Developer)
    };
    let sources = org_sources(&org);
    assert!(sources.iter().all(|(r, _)| org.effective().contains(*r)));
    assert!(sources.contains(&(OrgRight::LogsDelete, vec![Source::Added])));
    assert!(sources.contains(&(OrgRight::OrgView, vec![Source::Role])));
    assert!(!sources.iter().any(|(r, _)| *r == OrgRight::MembersView));
    let project = ProjectGrant {
        added: [ProjectRight::LogsExport].into_iter().collect(),
        ..ProjectGrant::preset(ProjectPreset::Viewer)
    };
    let p = project_sources(Some(&org), Some(&project));
    assert!(p.contains(&(
        ProjectRight::ProjectView,
        vec![Source::Role, Source::OrgRight(OrgRight::ProjectsViewAll)]
    )));
    assert!(p.contains(&(ProjectRight::LogsExport, vec![Source::Added])));
    let owner = OrgGrant::preset(OrgPreset::Owner);
    assert!(
        project_sources(Some(&owner), None)
            .iter()
            .all(|(_, s)| s == &[Source::Owner])
    );
    assert_eq!(
        project_sources(Some(&owner), None).len(),
        ProjectRight::ALL.len()
    );
    let admin = OrgGrant::preset(OrgPreset::Admin);
    assert!(
        project_sources(Some(&admin), None)
            .iter()
            .all(|(_, s)| s[0] == Source::OrgRole)
    );
}

#[test]
fn peers_and_changes_follow_the_grant_rules() {
    let admin = OrgHolding::of(&OrgGrant::preset(OrgPreset::Admin));
    let owner = OrgHolding::of(&OrgGrant::preset(OrgPreset::Owner));
    let member = OrgHolding::of(&OrgGrant::preset(OrgPreset::Member));
    let auditor = OrgHolding::of(&OrgGrant::preset(OrgPreset::Auditor));
    assert!(admin.covers(&admin) && admin.covers(&member) && admin.covers(&auditor));
    assert!(!admin.covers(&owner) && owner.covers(&admin));
    assert!(!member.covers(&auditor));
    let dev = OrgHolding::of(&OrgGrant {
        added: [OrgRight::MembersRightsUpdate].into_iter().collect(),
        ..OrgGrant::preset(OrgPreset::Developer)
    });
    assert!(!dev.covers(&admin));
    let err = dev.check_change(&member, &auditor).unwrap_err();
    assert!(matches!(err, GrantCheckError::Exceeds(_)));
    assert!(admin.check_change(&member, &auditor).is_ok());
    let without = OrgHolding {
        org: OrgPreset::Member.rights().with(OrgRight::LogsDelete),
        every_project: ProjectRights::EMPTY,
    };
    assert_eq!(
        admin
            .check_change(&member, &without)
            .unwrap_err()
            .to_string(),
        "`logs.delete` needs `logs.storage.view`."
    );
    let legacy = OrgHolding {
        org: OrgRights::EMPTY
            .with(OrgRight::OrgView)
            .with(OrgRight::LogsDelete),
        every_project: ProjectRights::EMPTY,
    };
    let legacy_plus = OrgHolding {
        org: legacy.org.with(OrgRight::MembersView),
        ..legacy
    };
    assert!(admin.check_change(&legacy, &legacy_plus).is_ok());
    let with_delete = OrgHolding {
        org: OrgPreset::Admin.rights(),
        every_project: ProjectRights::EMPTY,
    };
    let without_storage = OrgHolding {
        org: with_delete
            .org
            .minus(OrgRights::EMPTY.with(OrgRight::LogsStorageView)),
        ..with_delete
    };
    assert_eq!(
        admin
            .check_change(&with_delete, &without_storage)
            .unwrap_err()
            .to_string(),
        "`logs.delete` needs `logs.storage.view`; remove `logs.delete` too."
    );
    let without_both = OrgHolding {
        org: without_storage
            .org
            .minus(OrgRights::EMPTY.with(OrgRight::LogsDelete)),
        ..with_delete
    };
    assert!(admin.check_change(&with_delete, &without_both).is_ok());
    assert_eq!(
        check_project_change(
            ProjectPreset::Admin.rights(),
            ProjectPreset::Admin.rights(),
            ProjectPreset::Admin
                .rights()
                .minus(ProjectRights::EMPTY.with(ProjectRight::ModelsView))
        )
        .unwrap_err()
        .to_string(),
        "`models.manage` needs `models.view`; remove `models.manage` too."
    );
    assert!(
        check_project_change(
            ProjectPreset::Admin.rights(),
            ProjectRights::EMPTY,
            ProjectRights::EMPTY.with(ProjectRight::ModelsManage)
        )
        .is_err()
    );
}

#[test]
fn a_need_already_missing_does_not_hide_one_being_removed() {
    let before = OrgRights::parse([
        "org.update",
        "org.delete",
        "members.view",
        "members.invites.revoke",
        "members.rights.update",
        "projects.view.all",
        "provider_credentials.view",
        "provider_credentials.create",
        "provider_credentials.replace",
        "provider_credentials.delete",
        "security.sso.require",
        "security.2fa.require",
        "platform_keys.view",
        "platform_keys.create",
        "platform_keys.revoke",
        "logs.settings.manage",
        "logs.storage.view",
        "billing.view",
        "audit_log.export",
    ])
    .unwrap();
    let after = OrgRights::parse([
        "projects.view.all",
        "provider_credentials.view",
        "provider_credentials.create",
        "provider_credentials.delete",
        "security.2fa.require",
        "platform_keys.create",
        "platform_keys.revoke",
        "logs.settings.manage",
        "logs.storage.view",
        "billing.view",
    ])
    .unwrap();
    assert_eq!(
        check_org_needs(before, after).unwrap_err().to_string(),
        "`platform_keys.create` needs `platform_keys.view`; remove `platform_keys.create` too."
    );
    let kept = after.with(OrgRight::PlatformKeysView);
    assert!(check_org_needs(before, kept).is_ok());

    let before = ProjectRights::parse(["models.view", "models.manage"]).unwrap();
    let after = ProjectRights::parse(["models.manage"]).unwrap();
    assert_eq!(
        check_project_needs(before, after).unwrap_err().to_string(),
        "`models.manage` needs `models.view`; remove `models.manage` too."
    );
    assert!(check_project_needs(before, before).is_ok());
}

#[test]
fn a_role_change_never_strands_its_holders() {
    let org = |names: &[&str]| OrgRights::parse(names.iter().copied()).unwrap();
    let invites = Deviation {
        added: org(&["members.invite"]),
        removed: OrgRights::EMPTY,
    };
    let plain = Deviation::default();
    let before = org(&["org.view", "members.view"]);
    let after = org(&["org.view"]);
    assert_eq!(
        check_org_role_holders(before, after, &[plain, invites, plain, invites])
            .unwrap_err()
            .to_string(),
        "`members.invite` needs `members.view` for 2 of this role's holders; keep it in the role."
    );
    assert!(check_org_role_holders(before, after, &[plain, plain]).is_ok());
    assert!(check_org_role_holders(before, after, &[]).is_ok());
    assert!(check_org_role_holders(before, before, &[invites]).is_ok());
    let both = org(&["org.view", "members.view", "members.invite"]);
    assert!(check_org_role_holders(before, both, &[invites]).is_ok());

    let viewer_removed = Deviation {
        added: OrgRights::EMPTY,
        removed: org(&["members.view"]),
    };
    assert_eq!(
        check_org_role_holders(both, both, &[viewer_removed]),
        Ok(())
    );
    assert_eq!(
        check_org_role_holders(before, both, &[viewer_removed])
            .unwrap_err()
            .to_string(),
        "`members.invite` needs `members.view`, removed from 1 of this role's holders; restore it."
    );

    let project = |names: &[&str]| ProjectRights::parse(names.iter().copied()).unwrap();
    let manager = Deviation {
        added: project(&["models.manage"]),
        removed: ProjectRights::EMPTY,
    };
    assert_eq!(
        check_project_role_holders(
            project(&["project.view", "models.view"]),
            project(&["project.view"]),
            &[manager]
        )
        .unwrap_err(),
        StrandedHolders::Removed {
            right: "models.manage",
            needs: "models.view",
            count: 1
        }
    );
    assert!(
        check_project_role_holders(
            project(&["project.view"]),
            project(&["project.view", "usage.view"]),
            &[manager]
        )
        .is_ok()
    );
}

#[test]
fn deviations_add_and_remove() {
    let grant = ProjectGrant {
        added: [ProjectRight::LogsExport].into_iter().collect(),
        removed: [ProjectRight::LogsContentView].into_iter().collect(),
        ..ProjectGrant::preset(ProjectPreset::Developer)
    };
    let rights = grant.effective();
    assert!(rights.contains(ProjectRight::LogsExport));
    assert!(!rights.contains(ProjectRight::LogsContentView));
    assert!(rights.contains(ProjectRight::ApiRequest));
}

#[test]
fn owners_cannot_lose_rights() {
    let grant = OrgGrant {
        removed: [OrgRight::OrgDelete].into_iter().collect(),
        ..OrgGrant::preset(OrgPreset::Owner)
    };
    assert_eq!(grant.effective(), OrgRights::all());
    assert_eq!(grant.normalised(), OrgGrant::preset(OrgPreset::Owner));
}

#[test]
fn only_owners_hold_owner_only_rights() {
    assert_eq!(OrgRights::owner_only().names(), ["org.ownership.transfer"]);
    let id = crate::id::RoleId::new();
    let everything = OrgRights::all();
    for grant in [
        OrgGrant {
            added: everything,
            ..OrgGrant::preset(OrgPreset::Admin)
        },
        OrgGrant::custom(id, everything, ProjectRights::all()),
    ] {
        assert!(!grant.effective().contains(OrgRight::OrgOwnershipTransfer));
        assert!(
            !grant
                .normalised()
                .added
                .contains(OrgRight::OrgOwnershipTransfer)
        );
        assert_eq!(grant.effective(), everything.minus(OrgRights::owner_only()));
    }
    assert!(
        OrgGrant::preset(OrgPreset::Owner)
            .effective()
            .contains(OrgRight::OrgOwnershipTransfer)
    );
}

#[test]
fn normalising_drops_redundant_deviations() {
    let grant = OrgGrant {
        added: [OrgRight::OrgView, OrgRight::AuditLogView]
            .into_iter()
            .collect(),
        removed: [
            OrgRight::BillingManage,
            OrgRight::MembersView,
            OrgRight::AuditLogView,
        ]
        .into_iter()
        .collect(),
        ..OrgGrant::preset(OrgPreset::Member)
    }
    .normalised();
    assert!(grant.added.is_empty());
    assert_eq!(grant.removed.names(), ["members.view"]);
}

#[test]
fn grantors_cannot_grant_what_they_lack() {
    let admin = OrgPreset::Admin.rights();
    let member = OrgPreset::Member.rights();
    assert!(check_org_grant(admin, member, OrgPreset::Admin.rights()).is_ok());
    let err = check_org_grant(admin, member, OrgPreset::Owner.rights()).unwrap_err();
    assert_eq!(
        err.missing,
        ["org.delete", "org.ownership.transfer", "billing.manage"]
    );
    assert!(check_org_grant(member, admin, member).is_ok());
    let dev = ProjectPreset::Developer.rights();
    assert!(check_project_grant(dev, ProjectRights::EMPTY, ProjectPreset::Viewer.rights()).is_ok());
    let admin = ProjectPreset::Admin.rights();
    let err = check_project_grant(dev, ProjectRights::EMPTY, admin).unwrap_err();
    assert!(!err.missing.is_empty());
    assert_eq!(err.missing, admin.minus(dev).names());
}

#[test]
fn names_round_trip_and_unknown_names_fail() {
    for r in OrgRight::ALL {
        assert_eq!(r.as_str().parse::<OrgRight>().unwrap(), *r);
    }
    for r in ProjectRight::ALL {
        assert_eq!(r.as_str().parse::<ProjectRight>().unwrap(), *r);
    }
    assert_eq!(
        OrgRights::parse(["org.view", "org.fly"]).unwrap_err().0,
        "org.fly"
    );
    assert_eq!(
        ProjectRights::parse(["usage.view"]).unwrap().names(),
        ["usage.view"]
    );
    assert_eq!(
        "org.view".parse::<ProjectRight>().unwrap_err().0,
        "org.view"
    );
    for p in OrgPreset::ALL {
        assert_eq!(p.as_str().parse::<OrgPreset>().unwrap(), p);
    }
}

#[test]
fn sensitive_rights_need_a_recent_sign_in() {
    let org: Vec<&str> = OrgRight::ALL
        .iter()
        .filter(|r| r.needs_recent_sign_in())
        .map(|r| r.as_str())
        .collect();
    assert_eq!(
        org,
        [
            "org.delete",
            "org.ownership.transfer",
            "provider_credentials.create",
            "provider_credentials.replace",
            "security.sso.manage",
            "security.sso.require",
            "security.2fa.require",
            "platform_keys.create",
            "logs.delete"
        ]
    );
    let project: Vec<&str> = ProjectRight::ALL
        .iter()
        .filter(|r| r.needs_recent_sign_in())
        .map(|r| r.as_str())
        .collect();
    assert_eq!(project, ["keys.platform_access.grant"]);
}

#[test]
fn org_presets_map_to_their_default_project_presets() {
    use OrgPreset as O;
    use ProjectPreset as P;
    for (org, project) in [
        (O::Owner, P::Admin),
        (O::Admin, P::Admin),
        (O::Developer, P::Developer),
        (O::Billing, P::Viewer),
        (O::Auditor, P::Viewer),
        (O::Member, P::Viewer),
    ] {
        assert_eq!(org.default_project_preset(), project);
    }
}

struct Entry {
    name: &'static str,
    sensitive: bool,
    category: &'static str,
    label: &'static str,
    allows: &'static str,
    needs: Vec<&'static str>,
    recent_sign_in: bool,
    cloud_only: bool,
}

impl From<OrgRight> for Entry {
    fn from(r: OrgRight) -> Entry {
        Entry {
            name: r.as_str(),
            sensitive: r.is_sensitive(),
            category: r.category().name(),
            label: r.label(),
            allows: r.description(),
            needs: r.needs_beyond_view().iter().map(|n| n.as_str()).collect(),
            recent_sign_in: r.needs_recent_sign_in(),
            cloud_only: r.cloud_only(),
        }
    }
}

impl From<ProjectRight> for Entry {
    fn from(r: ProjectRight) -> Entry {
        Entry {
            name: r.as_str(),
            sensitive: r.is_sensitive(),
            category: r.category().name(),
            label: r.label(),
            allows: r.description(),
            needs: r.needs_beyond_view().iter().map(|n| n.as_str()).collect(),
            recent_sign_in: r.needs_recent_sign_in(),
            cloud_only: r.cloud_only(),
        }
    }
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn check(yes: bool) -> &'static str {
    if yes { "✓" } else { "" }
}

fn code_list(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn row(cells: &[String]) -> String {
    format!("| {} |", cells.join(" | "))
}

fn header(columns: &[&str], centred_from: usize) -> [String; 2] {
    let rule: Vec<&str> = (0..columns.len())
        .map(|i| if i < centred_from { "---" } else { ":-:" })
        .collect();
    [
        format!("| {} |", columns.join(" | ")),
        format!("|{}|", rule.join("|")),
    ]
}

fn rights_section(out: &mut Vec<String>, heading: &str, view: &str, entries: Vec<Entry>) {
    out.push(format!("## {heading}"));
    out.push(String::new());
    let needs = format!("Needs (beyond `{view}`)");
    out.extend(header(
        &[
            "Right",
            "Category",
            "Name",
            "Allows",
            &needs,
            "Recent sign-in",
            "Kaeso Cloud only",
        ],
        5,
    ));
    for e in entries {
        let right = if e.sensitive {
            format!("`{}` ⚠", e.name)
        } else {
            format!("`{}`", e.name)
        };
        out.push(row(&[
            right,
            cell(e.category),
            cell(e.label),
            cell(e.allows),
            code_list(&e.needs),
            check(e.recent_sign_in).to_owned(),
            check(e.cloud_only).to_owned(),
        ]));
    }
    out.push(String::new());
}

fn matrix_section(
    out: &mut Vec<String>,
    heading: &str,
    rights: &[&'static str],
    roles: &[(&'static str, Vec<&'static str>)],
) {
    out.push(format!("## {heading}"));
    out.push(String::new());
    let columns: Vec<&str> = std::iter::once("Right")
        .chain(roles.iter().map(|(name, _)| *name))
        .collect();
    out.extend(header(&columns, 1));
    for right in rights {
        let cells: Vec<String> = std::iter::once(format!("`{right}`"))
            .chain(
                roles
                    .iter()
                    .map(|(_, held)| check(held.contains(right)).to_owned()),
            )
            .collect();
        out.push(row(&cells));
    }
    out.push(String::new());
}

fn rights_reference() -> String {
    let org_rights: Vec<&'static str> = OrgRight::ALL.iter().map(|r| r.as_str()).collect();
    let project_rights: Vec<&'static str> = ProjectRight::ALL.iter().map(|r| r.as_str()).collect();
    let mut out = Vec::new();
    rights_section(
        &mut out,
        "Organization rights",
        OrgRight::OrgView.as_str(),
        OrgRight::ALL.iter().copied().map(Entry::from).collect(),
    );
    rights_section(
        &mut out,
        "Project rights",
        ProjectRight::ProjectView.as_str(),
        ProjectRight::ALL.iter().copied().map(Entry::from).collect(),
    );
    out.push("## Built-in organization roles".to_owned());
    out.push(String::new());
    out.extend(header(
        &["Role", "Id", "Description", "Default project role"],
        4,
    ));
    for p in OrgPreset::ALL {
        out.push(row(&[
            cell(p.name()),
            format!("`{}`", p.as_str()),
            cell(p.description()),
            cell(p.default_project_preset().name()),
        ]));
    }
    out.push(String::new());
    out.push("## Built-in project roles".to_owned());
    out.push(String::new());
    out.extend(header(&["Role", "Id", "Description"], 3));
    for p in ProjectPreset::ALL {
        out.push(row(&[
            cell(p.name()),
            format!("`{}`", p.as_str()),
            cell(p.description()),
        ]));
    }
    out.push(String::new());
    matrix_section(
        &mut out,
        "Organization roles",
        &org_rights,
        &OrgPreset::ALL.map(|p| (p.name(), p.rights().names())),
    );
    matrix_section(
        &mut out,
        "Rights in every project, by organization role",
        &project_rights,
        &OrgPreset::ALL.map(|p| (p.name(), p.project_rights().names())),
    );
    matrix_section(
        &mut out,
        "Project roles",
        &project_rights,
        &ProjectPreset::ALL.map(|p| (p.name(), p.rights().names())),
    );
    out.push("## Organization rights that reach into every project".to_owned());
    out.push(String::new());
    out.extend(header(&["Organization right", "Project rights"], 2));
    for r in OrgRight::ALL {
        let reach = org_reach(OrgRights::EMPTY.with(*r));
        if !reach.is_empty() {
            out.push(row(&[format!("`{r}`"), code_list(&reach.names())]));
        }
    }
    out.join("\n")
}

#[test]
fn the_rights_reference_is_current() {
    insta::assert_snapshot!("rights_reference", rights_reference());
}

fn org_rights() -> impl Strategy<Value = OrgRights> {
    any::<u64>().prop_map(|bits| {
        OrgRight::ALL
            .iter()
            .copied()
            .enumerate()
            .filter(|(i, _)| bits & (1 << i) != 0)
            .map(|(_, r)| r)
            .collect()
    })
}

fn role_rights() -> impl Strategy<Value = OrgRights> {
    org_rights().prop_map(|r| r.minus(OrgRights::owner_only()))
}

fn deviations() -> impl Strategy<Value = Vec<Deviation<OrgRights>>> {
    prop::collection::vec(
        (role_rights(), role_rights(), role_rights()).prop_map(|(a, b, c)| Deviation {
            added: a.intersect(b),
            removed: b.intersect(c).minus(a),
        }),
        0..5,
    )
}

proptest! {
    #[test]
    fn a_checked_grant_never_exceeds_the_grantor(grantor in org_rights(), before in org_rights(), after in org_rights()) {
        if check_org_grant(grantor, before, after).is_ok() {
            prop_assert!(after.minus(before).is_subset(grantor));
        }
    }

    #[test]
    fn a_checked_change_never_strands_a_right(before in org_rights(), after in org_rights()) {
        if check_org_needs(before, after).is_ok() {
            for r in after.iter() {
                for n in r.needs() {
                    let stranded_before = before.contains(r) && !before.contains(n);
                    prop_assert!(after.contains(n) || stranded_before);
                }
            }
        }
    }

    #[test]
    fn effective_rights_are_preset_plus_added_minus_removed(added in org_rights(), removed in org_rights(), p in 1usize..6) {
        let preset = OrgPreset::ALL[p];
        let g = OrgGrant { added, removed, ..OrgGrant::preset(preset) };
        let e = g.effective();
        prop_assert!(added.minus(removed).minus(OrgRights::owner_only()).is_subset(e));
        prop_assert!(e.intersect(OrgRights::owner_only()).is_empty());
        prop_assert!(e.minus(preset.rights().union(added)).is_empty());
        prop_assert_eq!(e.union(removed).minus(removed), e);
        prop_assert_eq!(g.normalised().effective(), e);
    }

    #[test]
    fn a_checked_role_change_never_strands_a_holder(before in role_rights(), after in role_rights(), holders in deviations()) {
        let id = crate::id::RoleId::new();
        let held = |role: OrgRights, d: &Deviation<OrgRights>| {
            OrgGrant { added: d.added, removed: d.removed, ..OrgGrant::custom(id, role, ProjectRights::EMPTY) }.effective()
        };
        let stranding = holders.iter().filter(|d| check_org_needs(held(before, d), held(after, d)).is_err()).count();
        match check_org_role_holders(before, after, &holders) {
            Ok(()) => prop_assert_eq!(stranding, 0),
            Err(StrandedHolders::Removed { count, .. } | StrandedHolders::Gained { count, .. }) => {
                prop_assert!(count >= 1 && count <= stranding);
            }
        }
    }
}
