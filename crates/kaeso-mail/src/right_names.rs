use crate::Locale;

// The console's names for each right (`apps/console/src/i18n/locales/*/rights.json`), so a mail
// names the right a reader recognises from the console; `tests::the_names_match_the_console` keeps them equal.
const NAMES: &[(&str, &str, &str, &str)] = &[
    (
        "account.keys.manage",
        "Manage personal keys",
        "Eigene Schlüssel verwalten",
        "管理个人密钥",
    ),
    (
        "account.keys.view",
        "View personal keys",
        "Eigene Schlüssel ansehen",
        "查看个人密钥",
    ),
    (
        "account.memberships.leave",
        "Leave organizations",
        "Organisationen verlassen",
        "退出组织",
    ),
    (
        "account.notifications.manage",
        "Change notifications",
        "Benachrichtigungen ändern",
        "更改通知",
    ),
    (
        "account.update",
        "Edit the profile",
        "Profil bearbeiten",
        "编辑个人资料",
    ),
    (
        "account.view",
        "View the account",
        "Konto ansehen",
        "查看账户",
    ),
    (
        "api.request",
        "Call models with keys",
        "Modelle mit Schlüsseln aufrufen",
        "使用密钥调用模型",
    ),
    (
        "audit_log.export",
        "Export audit log",
        "Audit-Log exportieren",
        "导出审计日志",
    ),
    (
        "audit_log.view",
        "View audit log",
        "Audit-Log ansehen",
        "查看审计日志",
    ),
    (
        "billing.manage",
        "Manage billing",
        "Abrechnung verwalten",
        "管理账单",
    ),
    (
        "billing.view",
        "View billing",
        "Abrechnung ansehen",
        "查看账单",
    ),
    (
        "costs.org.view",
        "View org costs",
        "Gesamtkosten ansehen",
        "查看组织费用",
    ),
    ("costs.view", "View costs", "Kosten ansehen", "查看费用"),
    (
        "instance.audit_log.view",
        "View instance audit log",
        "Instanz-Audit-Log ansehen",
        "查看实例审计日志",
    ),
    (
        "instance.orgs.manage",
        "Manage organizations",
        "Organisationen verwalten",
        "管理组织",
    ),
    (
        "instance.orgs.view",
        "View organizations",
        "Organisationen ansehen",
        "查看组织",
    ),
    (
        "instance.settings.manage",
        "Change instance settings",
        "Instanzeinstellungen ändern",
        "更改实例设置",
    ),
    (
        "instance.system.view",
        "View the system",
        "System ansehen",
        "查看系统",
    ),
    (
        "instance.updates.apply",
        "Apply updates",
        "Updates installieren",
        "安装更新",
    ),
    (
        "instance.updates.manage",
        "Manage updates",
        "Updates verwalten",
        "管理更新",
    ),
    (
        "instance.users.manage",
        "Manage users",
        "Benutzer verwalten",
        "管理用户",
    ),
    (
        "instance.users.view",
        "View users",
        "Benutzer ansehen",
        "查看用户",
    ),
    (
        "keys.all.manage",
        "Manage all keys",
        "Alle Schlüssel verwalten",
        "管理所有密钥",
    ),
    (
        "keys.all.view",
        "View all keys",
        "Alle Schlüssel ansehen",
        "查看所有密钥",
    ),
    (
        "keys.own.create",
        "Create own keys",
        "Eigene Schlüssel erstellen",
        "创建自己的密钥",
    ),
    (
        "keys.own.manage",
        "Manage own keys",
        "Eigene Schlüssel verwalten",
        "管理自己的密钥",
    ),
    (
        "keys.platform_access.grant",
        "Give keys platform access",
        "Plattformzugriff vergeben",
        "授予密钥平台访问权限",
    ),
    (
        "limits.manage",
        "Manage limits",
        "Limits verwalten",
        "管理限额",
    ),
    ("limits.view", "View limits", "Limits ansehen", "查看限额"),
    (
        "logs.content.manage",
        "Switch content logging",
        "Inhaltsprotokollierung schalten",
        "切换内容日志",
    ),
    (
        "logs.content.view",
        "View prompts and outputs",
        "Prompts und Ausgaben ansehen",
        "查看提示词和输出",
    ),
    (
        "logs.delete",
        "Delete logs",
        "Protokolle löschen",
        "删除日志",
    ),
    (
        "logs.export",
        "Export logs",
        "Protokolle exportieren",
        "导出日志",
    ),
    (
        "logs.metadata.view",
        "View request logs",
        "Anfrageprotokolle ansehen",
        "查看请求日志",
    ),
    (
        "logs.settings.manage",
        "Manage log settings",
        "Protokollierung verwalten",
        "管理日志设置",
    ),
    (
        "logs.storage.view",
        "View log storage",
        "Protokollspeicher ansehen",
        "查看日志存储",
    ),
    (
        "mcp_servers.manage",
        "Manage MCP servers",
        "MCP-Server verwalten",
        "管理 MCP 服务器",
    ),
    (
        "mcp_servers.view",
        "View MCP servers",
        "MCP-Server ansehen",
        "查看 MCP 服务器",
    ),
    (
        "members.invite",
        "Invite members",
        "Mitglieder einladen",
        "邀请成员",
    ),
    (
        "members.invites.revoke",
        "Cancel invites",
        "Einladungen zurückziehen",
        "取消邀请",
    ),
    (
        "members.remove",
        "Remove members",
        "Mitglieder entfernen",
        "移除成员",
    ),
    (
        "members.rights.update",
        "Change member roles",
        "Rollen zuweisen",
        "更改成员角色",
    ),
    (
        "members.view",
        "View members",
        "Mitglieder ansehen",
        "查看成员",
    ),
    (
        "models.manage",
        "Manage models",
        "Modelle verwalten",
        "管理模型",
    ),
    ("models.view", "View models", "Modelle ansehen", "查看模型"),
    (
        "org.delete",
        "Delete organization",
        "Organisation löschen",
        "删除组织",
    ),
    (
        "org.ownership.transfer",
        "Transfer ownership",
        "Inhaberschaft übertragen",
        "转让所有权",
    ),
    (
        "org.update",
        "Edit organization",
        "Organisation bearbeiten",
        "编辑组织",
    ),
    (
        "org.view",
        "View organization",
        "Organisation ansehen",
        "查看组织",
    ),
    (
        "platform_keys.create",
        "Create platform keys",
        "Plattform-Schlüssel erstellen",
        "创建平台密钥",
    ),
    (
        "platform_keys.revoke",
        "Disable platform keys",
        "Plattform-Schlüssel deaktivieren",
        "停用平台密钥",
    ),
    (
        "platform_keys.view",
        "View platform keys",
        "Plattform-Schlüssel ansehen",
        "查看平台密钥",
    ),
    (
        "playground.use",
        "Use the Playground",
        "Playground nutzen",
        "使用试验场",
    ),
    (
        "project.archive",
        "Archive project",
        "Projekt archivieren",
        "归档项目",
    ),
    (
        "project.audit_log.view",
        "View project audit log",
        "Projekt-Audit-Log ansehen",
        "查看项目审计日志",
    ),
    (
        "project.members.add",
        "Add members",
        "Mitglieder hinzufügen",
        "添加成员",
    ),
    (
        "project.members.remove",
        "Remove members",
        "Mitglieder entfernen",
        "移除成员",
    ),
    (
        "project.members.rights.update",
        "Change member roles",
        "Rollen zuweisen",
        "更改成员角色",
    ),
    (
        "project.members.view",
        "View members",
        "Mitglieder ansehen",
        "查看成员",
    ),
    (
        "project.update",
        "Edit project",
        "Projekt bearbeiten",
        "编辑项目",
    ),
    (
        "project.view",
        "View project",
        "Projekt ansehen",
        "查看项目",
    ),
    (
        "projects.archive.any",
        "Archive any project",
        "Beliebige Projekte archivieren",
        "归档任意项目",
    ),
    (
        "projects.create",
        "Create projects",
        "Projekte erstellen",
        "创建项目",
    ),
    (
        "projects.view.all",
        "View all projects",
        "Alle Projekte ansehen",
        "查看所有项目",
    ),
    (
        "provider_credentials.create",
        "Add provider keys",
        "Anbieter-Schlüssel hinzufügen",
        "添加服务商密钥",
    ),
    (
        "provider_credentials.delete",
        "Delete provider keys",
        "Anbieter-Schlüssel löschen",
        "删除服务商密钥",
    ),
    (
        "provider_credentials.replace",
        "Change provider keys",
        "Anbieter-Schlüssel ändern",
        "更改服务商密钥",
    ),
    (
        "provider_credentials.view",
        "View provider keys",
        "Anbieter-Schlüssel ansehen",
        "查看服务商密钥",
    ),
    (
        "providers.custom.manage",
        "Manage custom providers",
        "Eigene Anbieter verwalten",
        "管理自定义服务商",
    ),
    (
        "providers.custom.view",
        "View custom providers",
        "Eigene Anbieter ansehen",
        "查看自定义服务商",
    ),
    (
        "roles.manage",
        "Manage roles",
        "Rollen verwalten",
        "管理角色",
    ),
    (
        "security.2fa.require",
        "Require two-factor",
        "Zwei-Faktor verlangen",
        "要求双重验证",
    ),
    (
        "security.key_policy.manage",
        "Set key lifetime",
        "Schlüssellaufzeit festlegen",
        "设置密钥有效期",
    ),
    (
        "security.key_policy.project.manage",
        "Set key lifetime",
        "Schlüssellaufzeit festlegen",
        "设置密钥有效期",
    ),
    (
        "security.sso.manage",
        "Manage SSO",
        "SSO verwalten",
        "管理 SSO",
    ),
    (
        "security.sso.require",
        "Require SSO",
        "SSO verlangen",
        "要求 SSO",
    ),
    (
        "service_accounts.archive",
        "Archive service accounts",
        "Dienstkonten archivieren",
        "归档服务账号",
    ),
    (
        "service_accounts.create",
        "Create service accounts",
        "Dienstkonten erstellen",
        "创建服务账号",
    ),
    (
        "service_accounts.keys.create",
        "Create service account keys",
        "Dienstkonto-Schlüssel erstellen",
        "创建服务账号密钥",
    ),
    (
        "service_accounts.rights.update",
        "Change service accounts",
        "Dienstkonten ändern",
        "更改服务账号",
    ),
    (
        "service_accounts.view",
        "View service accounts",
        "Dienstkonten ansehen",
        "查看服务账号",
    ),
    (
        "tools.manage",
        "Manage tools",
        "Tools verwalten",
        "管理工具",
    ),
    ("tools.use", "Use tools", "Tools nutzen", "使用工具"),
    ("tools.view", "View tools", "Tools ansehen", "查看工具"),
    (
        "usage.org.view",
        "View org usage",
        "Gesamtnutzung ansehen",
        "查看组织用量",
    ),
    ("usage.view", "View usage", "Nutzung ansehen", "查看用量"),
];

pub(crate) fn right_name(lang: Locale, id: &str) -> Option<&'static str> {
    NAMES.iter().find(|row| row.0 == id).map(|row| match lang {
        Locale::En => row.1,
        Locale::De => row.2,
        Locale::ZhCn => row.3,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn console(lang: &str) -> serde_json::Map<String, serde_json::Value> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/console/src/i18n/locales")
            .join(lang)
            .join("rights.json");
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn key(id: &str) -> String {
        let mut parts = id.split(['.', '_']);
        let mut key = format!("rights.{}", parts.next().unwrap());
        for part in parts {
            let mut chars = part.chars();
            key.extend(chars.next().map(|c| c.to_ascii_uppercase()));
            key.push_str(chars.as_str());
        }
        key.push_str(".name");
        key
    }

    fn catalogue_ids() -> Vec<String> {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../kaeso-core/src/rights/catalogue.rs");
        let source = std::fs::read_to_string(path).unwrap();
        let mut ids: Vec<String> = source
            .lines()
            .filter_map(|line| {
                let (name, rest) = line.trim_start().split_once(" = \"")?;
                name.starts_with(|c: char| c.is_ascii_uppercase())
                    .then(|| rest.split('"').next().map(str::to_owned))
                    .flatten()
            })
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn every_right_of_the_catalogue_has_a_name() {
        let ours: Vec<String> = NAMES.iter().map(|row| row.0.to_owned()).collect();
        assert_eq!(ours, catalogue_ids());
    }

    #[test]
    fn the_names_match_the_console() {
        for (lang, catalog) in [("en", 1), ("de", 2), ("zh-CN", 3)] {
            let names = console(lang);
            for row in NAMES {
                let ours = [row.1, row.2, row.3][catalog - 1];
                assert_eq!(
                    names.get(&key(row.0)).and_then(|v| v.as_str()),
                    Some(ours),
                    "{lang} {}",
                    row.0
                );
            }
        }
    }

    #[test]
    fn an_unknown_right_has_no_name() {
        assert_eq!(
            right_name(Locale::De, "provider_credentials.replace"),
            Some("Anbieter-Schlüssel ändern")
        );
        assert_eq!(right_name(Locale::En, "no.such.right"), None);
    }
}
