---
title: SCIM provisioning
description: Let your identity provider add and remove members through SCIM 2.0, with their keys archived when they leave.
order: 13
---

An organization with single sign-on can let its identity provider add and remove members through SCIM 2.0 ([RFC 7644](https://datatracker.ietf.org/doc/html/rfc7644), schemas in [RFC 7643](https://datatracker.ietf.org/doc/html/rfc7643)). When someone leaves the company and the provider deactivates them, Kaeso removes them from the organization and its projects and archives their API keys there, without anyone opening the console.

## Set it up

1. Connect single sign-on and verify your email domains ([single sign-on](../console/organizations-and-projects.md#single-sign-on)). Choose the role people join with under **Can people join without an invitation?**: provisioned members get it.
2. Under **Security** › **Provisioning**, choose **Create token** and copy it. It's shown once, and Kaeso keeps only its hash. This needs the **Manage SSO** right and a recent sign-in; agents and platform keys can't.
3. At your identity provider, give the app the **SCIM base URL** from the same section, `https://kaeso.example.com/platform/v1/orgs/{org}/scim/v2`, and the token as an HTTP bearer token.

**Okta**: in the app's **Provisioning** tab, choose **SCIM 2.0**, enter the base URL, set **Unique identifier field for users** to `userName` and authentication to **HTTP Header**, and turn on **Push New Users**, **Push Profile Updates** and **Push Groups** ([Okta's SCIM guide](https://developer.okta.com/docs/api/openapi/okta-scim/guides/scim-20)).

**Microsoft Entra ID**: in the enterprise application's **Provisioning**, set the mode to **Automatic**, enter the base URL as **Tenant URL** and the token as **Secret Token**, then **Test Connection** ([Entra ID's SCIM guide](https://learn.microsoft.com/en-us/entra/identity/app-provisioning/use-scim-to-provision-users-and-groups)).

**Replace token** makes a new one; the one it replaces keeps working for 24 hours, so you can update the provider. **Delete** stops provisioning at once; members stay.

## What it does

SCIM manages only accounts at the organization's verified domains. A guest, whose address is at another domain, is never listed, added or removed. Service accounts are never touched.

| The provider | Kaeso |
| --- | --- |
| Creates a user | Creates the account if there is none, and adds the membership with the single sign-on default role. The person's first sign-in through the provider links their identity. An existing account at a verified domain joins too |
| Changes a user's name | Changes the account's name. The address can't change through SCIM |
| Sets `active` to false | Removes the member from the organization and its projects, archives their keys in it, and ends the sessions the organization's provider signed in, as **Remove from organization** does. In no organization any more, all their sessions end |
| Sets `active` to true again | Adds a new membership with the default role. Archived keys stay archived |
| Deletes a user | Removes the member as above, and forgets that the provider manages them |
| Pushes groups | Keeps the groups and their members. They grant no access yet |

The organization's last owner is never removed: the provider gets `409` with a `detail` that says so. The audit log records `member.provision` and `member.deprovision` with the user's `externalId`, and the **Member changes** email goes out as for other members.

**Security** › **Provisioning** shows when the provider last called, how many members it keeps active, and the last request Kaeso refused, with its reason.

## Endpoints

Every request carries the organization's token as `Authorization: Bearer …`. A wrong token, or one of another organization, gets `401`, and repeated failures from one address are throttled like unknown API keys. Answers are `application/scim+json`; errors follow RFC 7644, such as `{"schemas": ["urn:ietf:params:scim:api:messages:2.0:Error"], "status": "409", "scimType": "uniqueness", "detail": "…"}`.

| Method and path | What it does |
| --- | --- |
| `GET /ServiceProviderConfig`, `GET /ResourceTypes`, `GET /Schemas` | What Kaeso supports: `PATCH` and filters, no bulk, sorting or ETags |
| `GET /Users` | Lists the members at verified domains and the users the provider deactivated. Filters `userName eq "…"` and `externalId eq "…"`; `startIndex` and `count`, at most 200 |
| `POST /Users` | Creates a user: `userName` (the email address), `externalId`, `name`, `displayName`, `emails`, `active` |
| `GET`, `PUT`, `PATCH`, `DELETE /Users/{id}` | Reads, replaces, changes or deletes a user. The id is the Kaeso user id, such as `usr_…` |
| `GET /Groups`, `POST /Groups` | Lists or creates groups; filters `displayName eq` and `externalId eq` |
| `GET`, `PUT`, `PATCH`, `DELETE /Groups/{id}` | Reads, replaces, changes or deletes a group |

`PATCH` takes operations as the providers send them: `op` in any case, with or without a `path`, and `active` as a boolean or the string `"True"` or `"False"`. Attributes Kaeso keeps nothing of, such as a title or the enterprise extension, are accepted and left out.

The Platform API manages the token: `GET /orgs/{org}/scim`, `POST /orgs/{org}/scim/token` and `DELETE /orgs/{org}/scim/token` ([Platform API](platform-api.md)).
