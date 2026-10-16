UPDATE org_memberships
SET rights_added = array_remove(rights_added, 'members.2fa.reset'),
    rights_removed = array_remove(rights_removed, 'members.2fa.reset')
WHERE 'members.2fa.reset' = ANY (rights_added)
   OR 'members.2fa.reset' = ANY (rights_removed);

UPDATE org_roles
SET rights = array_remove(rights, 'members.2fa.reset')
WHERE 'members.2fa.reset' = ANY (rights);
