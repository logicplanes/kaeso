import { screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { apply, load, type Locale } from "~/i18n";
import german from "~/i18n/locales/de/rights.json";
import { ORG, PROJECT, catalogue, signedIn } from "~/test/fixtures";
import { member, paged } from "~/test/fixtures/org";
import { on } from "~/test/network";
import { renderApp } from "~/test/render";
import { api, data } from "./client";
import { rightEnglish, translateBody } from "./vocabulary";

const speak = async (l: Locale) => {
  apply(await load(l));
  setFormatLocale(l);
};
afterEach(() => speak("en"));

const camel = (id: string) => id.replace(/[._]([a-z0-9])/g, (_, c: string) => c.toUpperCase());
const deleted = () =>
  member({
    user: { id: "usr_gone", name: "Deleted user", email: null },
    grant: { role: { id: "deleted", kind: "custom", name: "Deleted role" }, rights: [], project_rights: [], rights_added: [], rights_removed: [] },
  });

describe("the gateway's words in a German session", () => {
  it("name the built-in roles in German on the roles page", async () => {
    signedIn();
    on("GET", "orgs/:org/members", paged([]));
    await renderApp(`/settings/orgs/${ORG}/members?tab=roles`, { locale: "de" });
    expect((await screen.findAllByText("Inhaber")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("Entwickler").length).toBeGreaterThan(0);
  });

  it("name and describe every right of the catalogue in German", async () => {
    await speak("de");
    on("GET", "orgs/:org/rights", catalogue);
    const got = data(await api.GET("/platform/v1/orgs/{org}/rights", { params: { path: { org: ORG } } }));
    const catalog: Record<string, string> = german;
    const rights = [...got.org.categories, ...got.project.categories].flatMap((c) => c.rights);
    expect(rights.length).toBeGreaterThan(60);
    for (const right of rights) {
      const [englishName] = rightEnglish(right.id);
      expect(right.name, right.id).toBe(catalog[`rights.${camel(right.id)}.name`]);
      expect(right.name, right.id).not.toBe(englishName);
      expect(right.description, right.id).toBe(catalog[`rights.${camel(right.id)}.description`]);
    }
  });

  it("name deleted people and roles in German, except in logged records", async () => {
    await speak("de");
    on("GET", "orgs/:org/members", paged([deleted()]));
    on("GET", "orgs/:org/projects/:project/audit-log", paged([{ actor_name: "Deleted user", id: "evt_1", name: "Deleted user" }]));
    const members = data(await api.GET("/platform/v1/orgs/{org}/members", { params: { path: { org: ORG } } })).data;
    expect(members[0]?.user.name).toBe("Gelöschter Benutzer");
    expect(members[0]?.grant.role.name).toBe("Gelöschte Rolle");
    const logged = data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/audit-log", { params: { path: { org: ORG, project: PROJECT } } }));
    expect(JSON.stringify(logged)).toContain("Deleted user");
  });

  it("translate a status message the console knows", async () => {
    await speak("de");
    on("POST", "auth/magic-link", { status: "sent", message: "No such project, or you cannot see it." });
    const sent = data(await api.POST("/platform/v1/auth/magic-link", { body: { email: "ada@example.com" } }));
    expect(sent).toEqual({ status: "sent", message: "Projekt nicht gefunden oder nicht sichtbar." });
  });

  it("leave names and messages it doesn't know as the gateway wrote them", async () => {
    await speak("de");
    const body = [
      { id: "org.view", name: "Look around", description: "Our own words.", needs: [] },
      { id: "organization", name: "Everything else", rights: [] },
      { id: "owner", kind: "builtin", scope: "org", name: "Boss", description: "Runs the place." },
      { id: "nobody", kind: "builtin", scope: "org", name: "Nobody" },
    ];
    expect(await translateBody(body)).toBe(false);
    expect(body.map((n) => n.name)).toEqual(["Look around", "Everything else", "Boss", "Nobody"]);
    const status = { status: "sent", message: "Something the console has never seen." };
    expect(await translateBody(status)).toBe(false);
    expect(status.message).toBe("Something the console has never seen.");
  });
});
