UPDATE org_roles
SET rights = array_remove(rights, 'org.ownership.transfer'), updated_at = now()
WHERE 'org.ownership.transfer' = ANY (rights);

UPDATE org_memberships
SET rights_added = array_remove(rights_added, 'org.ownership.transfer'),
    rights_removed = array_remove(rights_removed, 'org.ownership.transfer'),
    updated_at = now()
WHERE 'org.ownership.transfer' = ANY (rights_added) OR 'org.ownership.transfer' = ANY (rights_removed);

UPDATE org_roles
SET rights = array_remove(rights, 'logs.content.disable'),
    project_rights = array_remove(project_rights, 'logs.content.disable'),
    updated_at = now()
WHERE 'logs.content.disable' = ANY (rights) OR 'logs.content.disable' = ANY (project_rights);

UPDATE project_memberships
SET rights_added = array_remove(rights_added, 'logs.content.disable'),
    rights_removed = array_replace(rights_removed, 'logs.content.disable', 'logs.content.manage'),
    updated_at = now()
WHERE 'logs.content.disable' = ANY (rights_added) OR 'logs.content.disable' = ANY (rights_removed);

UPDATE service_accounts
SET rights_added = array_remove(rights_added, 'logs.content.disable'),
    rights_removed = array_replace(rights_removed, 'logs.content.disable', 'logs.content.manage'),
    updated_at = now()
WHERE 'logs.content.disable' = ANY (rights_added) OR 'logs.content.disable' = ANY (rights_removed);
