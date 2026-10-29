import { afterEach, describe, expect, it, vi, type Mock } from "vitest";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { apply, load, type Locale } from "~/i18n";
import englishErrors from "~/i18n/locales/en/errors.json";
import { agentRightIds } from "./vocabulary-agents";

let fetchMock: Mock = vi.fn();
vi.stubGlobal("fetch", (...args: unknown[]) => fetchMock(...args) as unknown);
const { api, data } = await import("./client");
const { errorFromResponse } = await import("./errors");
const { pattern, rules } = await import("./gateway-errors");
const { knownRights, rightEnglish } = await import("./vocabulary");

const speak = async (l: Locale) => {
  apply(await load(l));
  setFormatLocale(l);
};
afterEach(() => speak("en"));

const failure = (message: string, status = 400) => new Response(JSON.stringify({ error: { type: "invalid_request", message } }), { status });
const answer = (body: unknown) => {
  fetchMock = vi.fn(async () => new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } }));
};

describe("the gateway's messages", () => {
  it("have rules whose English is the source's, so each pattern matches what the gateway writes", () => {
    const catalog: Record<string, string> = englishErrors;
    const all = rules();
    expect(all.length).toBeGreaterThan(300);
    for (const r of all) {
      const english = catalog[r.key] ?? "";
      const values = Object.fromEntries([...english.matchAll(/\{(\w+)\}/g)].map(([, name]) => [name, `‹${name}›`]));
      const written = english.replace(/\{(\w+)\}/g, "‹$1›");
      expect(r.text(values), r.key).toBe(written);
      expect(pattern(english).test(written), r.key).toBe(true);
    }
  });

  it("are shown in German with the values the gateway wrote", async () => {
    await speak("de");
    expect((await errorFromResponse(failure("Too many failed sign-ins; try again in 15 minutes or use a sign-in link.", 429))).message).toBe(
      "Zu viele fehlgeschlagene Anmeldungen; in 15 Min. erneut versuchen oder einen Anmeldelink nutzen.",
    );
    expect((await errorFromResponse(failure("No such project, or you cannot see it.", 404))).message).toBe("Projekt nicht gefunden oder nicht sichtbar.");
    expect((await errorFromResponse(failure("Several providers serve `gpt-5` here; name it as `provider/gpt-5`."))).message).toBe(
      "Mehrere Anbieter bieten `gpt-5` hier an; als `provider/gpt-5` angeben.",
    );
  });

  it("say how long to wait in the unit the gateway wrote, in every language", async () => {
    const wait = async (w: string) => (await errorFromResponse(failure(`Too many attempts from your address; try again in ${w}.`, 429))).message;
    expect(await wait("1 minute")).toBe("Too many attempts from your address; try again in 1 minute.");
    expect(await wait("3 seconds")).toBe("Too many attempts from your address; try again in 3 seconds.");
    await speak("de");
    expect(await wait("3 seconds")).toBe("Zu viele Versuche von Ihrer Adresse; in 3 Sek. erneut versuchen.");
    expect(await wait("2 hours")).toBe("Zu viele Versuche von Ihrer Adresse; in 2 Std. erneut versuchen.");
    await speak("zh-CN");
    expect(await wait("1 minute")).toBe("来自您地址的尝试次数过多，请 1 分钟后重试。");
    expect(await wait("soon")).toBe("Too many attempts from your address; try again in soon.");
  });

  it("name the provider that does not serve a model, in every language", async () => {
    const unserved = "Together AI does not serve `meta-llama/Llama-3.3-70B` to this project's key; check the key's access.";
    await speak("de");
    expect((await errorFromResponse(failure(unserved, 404))).message).toBe(
      "Together AI bietet `meta-llama/Llama-3.3-70B` dem Schlüssel dieses Projekts nicht an; Zugriff des Schlüssels prüfen.",
    );
    await speak("zh-CN");
    expect((await errorFromResponse(failure(unserved, 404))).message).toBe(
      "Together AI 不向此项目的密钥提供 `meta-llama/Llama-3.3-70B`；请检查密钥的访问权限。",
    );
  });

  it("are shown in Chinese", async () => {
    await speak("zh-CN");
    expect((await errorFromResponse(failure("ada@example.com is already a member of this organization.", 409))).message).toBe(
      "ada@example.com 已是此组织的成员。",
    );
  });

  it("translate the limit a refusal names, its owner and its amount", async () => {
    await speak("de");
    const refusal = "Budget used up: the project's monthly spend budget of 50 USD for this model; it does not reset, so raise the limit to continue.";
    expect((await errorFromResponse(failure(refusal, 429))).message).toBe(
      "Budget des Projekts aufgebraucht (Monats-Ausgabenbudget von 50 USD für dieses Modell); ohne Zurücksetzen – zum Fortfahren das Limit erhöhen.",
    );
    const above = "This limit is higher than the request rate of 1200 requests per minute of member usr_01 (1200).";
    expect((await errorFromResponse(failure(above))).message).toBe(
      "Dieses Limit liegt über einem übergeordneten Limit (Mitglied usr_01: Anfragerate von 1.200 Anfragen pro Minute; Betrag 1.200).",
    );
  });

  it("stay as the gateway wrote them when the console doesn't know them", async () => {
    await speak("de");
    expect((await errorFromResponse(failure("A brand new refusal.", 409))).message).toBe("A brand new refusal.");
    expect((await errorFromResponse(failure("Budget used up: the project's weekly budget of 3 apples; it resets soon.", 429))).message).toBe(
      "Budget used up: the project's weekly budget of 3 apples; it resets soon.",
    );
  });

  it("are left alone in English, and the console's own words follow the language", async () => {
    expect((await errorFromResponse(failure("No such project, or you cannot see it.", 404))).message).toBe("No such project, or you cannot see it.");
    await speak("de");
    expect((await errorFromResponse(new Response("teapot", { status: 418 }))).message).toBe("Das Gateway hat mit 418 geantwortet.");
  });
});

const catalogue = {
  org: {
    categories: [
      {
        id: "logs",
        name: "Logs & data",
        rights: [
          {
            id: "logs.delete",
            scope: "org",
            name: "Delete logs",
            description: "Delete the organization's logs for a time range.",
            sensitive: true,
            recent_sign_in: true,
            cloud_only: false,
            owner_only: false,
            needs: ["org.view", "logs.storage.view"],
          },
          {
            id: "logs.archive",
            scope: "org",
            name: "Archive logs",
            description: "A right from a newer gateway.",
            sensitive: false,
            recent_sign_in: false,
            cloud_only: false,
            owner_only: false,
            needs: [],
          },
        ],
      },
    ],
  },
  project: {
    categories: [
      {
        id: "usage",
        name: "Usage & costs",
        rights: [
          {
            id: "logs.export",
            scope: "project",
            name: "Export logs",
            description: "A description the gateway changed.",
            sensitive: true,
            recent_sign_in: false,
            cloud_only: false,
            owner_only: false,
            needs: [],
          },
        ],
      },
    ],
  },
};

describe("the gateway's words in its answers", () => {
  it("translate the rights catalogue by id, and keep what the console doesn't know", async () => {
    await speak("de");
    answer(catalogue);
    const got = data(await api.GET("/platform/v1/orgs/{org}/rights", { params: { path: { org: "org_1" } } }));
    const [logs] = got.org.categories;
    expect(logs?.name).toBe("Protokolle & Daten");
    expect(logs?.rights[0]).toMatchObject({
      id: "logs.delete",
      name: "Protokolle löschen",
      description: "Protokolle der Organisation für einen Zeitraum löschen.",
    });
    expect(logs?.rights[1]).toMatchObject({ name: "Archive logs", description: "A right from a newer gateway." });
    const [usage] = got.project.categories;
    expect(usage?.name).toBe("Nutzung & Kosten");
    expect(usage?.rights[0]).toMatchObject({ name: "Protokolle exportieren", description: "A description the gateway changed." });
  });

  it("translate built-in roles and deleted names, never custom roles", async () => {
    await speak("zh-CN");
    answer({
      data: [
        {
          id: "owner",
          scope: "org",
          kind: "builtin",
          name: "Owner",
          description: "Holds every right, now and in future releases; only owners manage owners.",
          rights: [],
          project_rights: [],
          holders: 1,
          created_at: null,
          updated_at: null,
        },
        {
          id: "admin",
          scope: "project",
          kind: "builtin",
          name: "Admin",
          description: "Manages the project: members, keys, service accounts, models, limits, logs and settings.",
          rights: [],
          project_rights: [],
          holders: 0,
          created_at: null,
          updated_at: null,
        },
        {
          id: "role_01",
          scope: "org",
          kind: "custom",
          name: "Owner",
          description: "Holds every right, now and in future releases; only owners manage owners.",
          rights: [],
          project_rights: [],
          holders: 0,
          created_at: null,
          updated_at: null,
        },
      ],
    });
    const roles = data(await api.GET("/platform/v1/orgs/{org}/roles", { params: { path: { org: "org_1" } } })).data;
    expect(roles.map((r) => [r.name, r.description])).toEqual([
      ["所有者", "拥有所有权限，包括未来版本新增的权限；只有所有者可以管理所有者。"],
      ["管理员", "管理项目：成员、密钥、服务账号、模型、限额、日志和设置。"],
      ["Owner", "Holds every right, now and in future releases; only owners manage owners."],
    ]);
  });

  it("are left as the gateway wrote them in English", async () => {
    answer(catalogue);
    const got = data(await api.GET("/platform/v1/orgs/{org}/rights", { params: { path: { org: "org_1" } } }));
    expect(got).toEqual(catalogue);
  });

  it("cover every right of the catalogue with the source's English", () => {
    for (const id of knownRights()) {
      const [name, description] = rightEnglish(id);
      expect(name, id).toBeTruthy();
      expect(description, id).toBeTruthy();
    }
    expect(knownRights()).toHaveLength(66);
    for (const id of agentRightIds()) expect(rightEnglish(id)[0], id).toBeTruthy();
    expect(agentRightIds()).toHaveLength(16);
  });
});
