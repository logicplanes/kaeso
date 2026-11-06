import type { NotificationDelivery, NotificationKindView, NotificationSettings, TestEmailResult } from "~/api/generated/types";

function kind(
  key: string,
  name: string,
  description: string,
  delivery: NotificationDelivery,
  options: { always?: boolean; reason?: string | null; default?: NotificationDelivery } = {},
): NotificationKindView {
  return {
    kind: key,
    name,
    description,
    always: options.always ?? false,
    default: options.default ?? delivery,
    delivery,
    reason: options.reason ?? null,
  };
}

export function notificationSettings(
  options: { sends_email?: boolean; deliveries?: Readonly<Record<string, NotificationDelivery>> } = {},
): NotificationSettings {
  const d = (key: string, fallback: NotificationDelivery) => options.deliveries?.[key] ?? fallback;
  return {
    sends_email: options.sends_email ?? true,
    areas: [
      {
        area: "security",
        name: "Security",
        kinds: [
          kind("account_security", "Account security", "Your password, two-factor authentication, email address or sessions changed.", "now", { always: true }),
          kind("new_app", "A new app can act as you", "You let an app or an MCP client act as you.", "now", { always: true }),
        ],
      },
      {
        area: "project",
        name: "Project",
        kinds: [
          kind("budget_alerts", "Budget alerts", "A budget you're responsible for crosses a threshold.", d("budget_alerts", "now"), {
            reason: "limits.manage",
            default: "now",
          }),
          kind("key_expiry", "Your key expires", "A key you own expires soon.", d("key_expiry", "now"), { reason: "keys.own.create", default: "now" }),
        ],
      },
      {
        area: "organization",
        name: "Organization",
        kinds: [
          kind("org_members", "Member changes", "People join or leave an organization.", d("org_members", "daily"), {
            reason: "members.invite",
            default: "daily",
          }),
        ],
      },
    ],
  };
}

export const testEmailSent: TestEmailResult = { status: "sent", to: "owner@example.com" };
