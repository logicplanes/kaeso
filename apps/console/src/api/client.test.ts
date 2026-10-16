import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { apply, load, type Locale } from "~/i18n";
import { apiError, json, noContent, on, requests } from "~/test/network";
import { api, data } from "./client";
import { ApiError, isApiError } from "./errors";

const speak = async (l: Locale) => {
  apply(await load(l));
  setFormatLocale(l);
};
afterEach(() => setFormatLocale("en"));

const org = { params: { path: { org: "org_1" } } };

async function failure(call: Promise<unknown>): Promise<ApiError> {
  const error: unknown = await call.then(
    () => undefined,
    (e: unknown) => e,
  );
  if (!isApiError(error)) throw new Error(`expected an ApiError, got ${String(error)}`);
  return error;
}

const deletedUser = { id: "usr_gone", name: "Deleted user", email: null };
const builtinOwner = {
  id: "owner",
  scope: "org",
  kind: "builtin",
  name: "Owner",
  description: "Holds every right, now and in future releases; only owners manage owners.",
};

describe("the client's answers", () => {
  it("hands over the body the gateway sent", async () => {
    on("GET", "orgs/:org", { id: "org_1", name: "Northwind Labs" });
    expect(data(await api.GET("/platform/v1/orgs/{org}", org))).toEqual({ id: "org_1", name: "Northwind Labs" });
    expect(requests("GET", "orgs/org_1")).toHaveLength(1);
  });

  it("refuses an empty answer where a body was due, in the reader's language", async () => {
    on("GET", "orgs/:org", noContent());
    const english = await failure(api.GET("/platform/v1/orgs/{org}", org).then(data));
    expect(english).toMatchObject({ type: "internal", status: 0, message: "The gateway sent an empty answer. Try again." });
    await speak("de");
    on("GET", "orgs/:org", noContent());
    expect((await failure(api.GET("/platform/v1/orgs/{org}", org).then(data))).message).toBe(
      "Das Gateway hat eine leere Antwort gesendet. Bitte erneut versuchen.",
    );
  });
});

describe("the client's errors", () => {
  it("carry the gateway's type, status, message, field, missing right and request id", async () => {
    on("DELETE", "orgs/:org/members/:user", () => {
      const response = apiError(403, "permission_denied", "You need the right `members.remove` in this organization; ask one of its admins for it.", {
        param: "user",
        right: "members.remove",
      });
      response.headers.set("kaeso-request-id", "req_01");
      return response;
    });
    const error = await failure(api.DELETE("/platform/v1/orgs/{org}/members/{user}", { params: { path: { org: "org_1", user: "usr_02" } } }));
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({
      name: "ApiError",
      type: "permission_denied",
      status: 403,
      message: "You need the right `members.remove` in this organization; ask one of its admins for it.",
      param: "user",
      right: "members.remove",
      requestId: "req_01",
      unreachable: false,
    });
  });

  it("say the gateway can't be reached when the network fails", async () => {
    on("GET", "orgs/:org", () => {
      throw new TypeError("Failed to fetch");
    });
    const error = await failure(api.GET("/platform/v1/orgs/{org}", org));
    expect(error).toMatchObject({ type: "network", status: 0, message: "Can't reach the gateway. Try again.", unreachable: true });
  });

  it("say so in Chinese when the network fails there", async () => {
    await speak("zh-CN");
    on("GET", "orgs/:org", () => {
      throw new TypeError("Failed to fetch");
    });
    expect((await failure(api.GET("/platform/v1/orgs/{org}", org))).message).toBe("无法连接网关，请重试。");
  });

  it("keep an error the fetch itself raised as one of the console's", async () => {
    const own = new ApiError({ type: "unavailable", status: 503, message: "Kaeso is restarting and ended this request; send it again." });
    on("GET", "orgs/:org", () => {
      throw own;
    });
    expect(await failure(api.GET("/platform/v1/orgs/{org}", org))).toBe(own);
  });

  it("say a bad gateway or timeout means it can't be reached, and name any other bare status", async () => {
    on("GET", "orgs/:org", new Response("<html>Bad Gateway</html>", { status: 502 }));
    expect(await failure(api.GET("/platform/v1/orgs/{org}", org))).toMatchObject({ type: "internal", message: "Can't reach the gateway. Try again." });
    on("GET", "orgs/:org", new Response(null, { status: 504 }));
    expect((await failure(api.GET("/platform/v1/orgs/{org}", org))).message).toBe("Can't reach the gateway. Try again.");
    on("GET", "orgs/:org", new Response("oops", { status: 500 }));
    expect(await failure(api.GET("/platform/v1/orgs/{org}", org))).toMatchObject({ type: "internal", status: 500, message: "The gateway answered 500." });
    on("GET", "orgs/:org", json({ detail: "no" }, { status: 422 }));
    expect(await failure(api.GET("/platform/v1/orgs/{org}", org))).toMatchObject({ type: "invalid_request", message: "The gateway answered 422." });
  });

  it("translate the gateway's message in German and keep its request id", async () => {
    await speak("de");
    on("GET", "orgs/:org/projects/:project", () => {
      const response = apiError(404, "not_found", "No such project, or you cannot see it.");
      response.headers.set("kaeso-request-id", "req_02");
      return response;
    });
    const error = await failure(api.GET("/platform/v1/orgs/{org}/projects/{project}", { params: { path: { org: "org_1", project: "proj_x" } } }));
    expect(error).toMatchObject({ type: "not_found", message: "Projekt nicht gefunden oder nicht sichtbar.", requestId: "req_02" });
  });
});

describe("the gateway's words in a German or Chinese console", () => {
  it("are translated in German answers", async () => {
    await speak("de");
    on("GET", "orgs/:org/roles", { data: [builtinOwner] });
    const [role] = data(await api.GET("/platform/v1/orgs/{org}/roles", org)).data;
    expect(role).toMatchObject({ id: "owner", name: "Inhaber" });
    expect(role?.description).not.toBe(builtinOwner.description);
  });

  it("are translated in Chinese answers, deleted people included", async () => {
    await speak("zh-CN");
    on("GET", "orgs/:org/members", { data: [{ user: deletedUser }], has_more: false, next_cursor: null });
    const got = data(await api.GET("/platform/v1/orgs/{org}/members", org)) as unknown as { data: { user: typeof deletedUser }[] };
    expect(got.data[0]?.user.name).toBe("已删除用户");
  });

  it("translate the message of an answer that only says what happens next", async () => {
    await speak("de");
    on("POST", "auth/magic-link", { message: "If an account exists for this address, we sent it a sign-in link.", status: "email_sent" });
    expect(data(await api.POST("/platform/v1/auth/magic-link", { body: { email: "ada@example.com" } }))).toEqual({
      message: "Falls es ein Konto mit dieser Adresse gibt, ist ein Anmeldelink unterwegs.",
      status: "email_sent",
    });
  });

  it("stay as the gateway wrote them in English", async () => {
    on("GET", "orgs/:org/roles", { data: [builtinOwner] });
    expect(data(await api.GET("/platform/v1/orgs/{org}/roles", org)).data[0]?.name).toBe("Owner");
  });

  it("stay as logged in logs, audit logs and the Playground", async () => {
    await speak("de");
    const logged = { data: [{ id: "evt_1", actor: deletedUser, target: builtinOwner }], has_more: false, next_cursor: null };
    on("GET", "orgs/:org/audit-log", logged);
    on("GET", "orgs/:org/projects/:project/logs", logged);
    on("GET", "orgs/:org/projects/:project/playground/models", logged);
    const project = { params: { path: { org: "org_1", project: "proj_1" } } };
    expect(data(await api.GET("/platform/v1/orgs/{org}/audit-log", org))).toEqual(logged);
    expect(data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/logs", project))).toEqual(logged);
    expect(data(await api.GET("/platform/v1/orgs/{org}/projects/{project}/playground/models", project))).toEqual(logged);
  });

  it("leave answers that aren't JSON, that can't be read, or that have nothing to translate untouched", async () => {
    await speak("de");
    on("GET", "orgs/:org", new Response(JSON.stringify(builtinOwner), { status: 200, headers: { "content-type": "text/plain" } }));
    expect(await api.GET("/platform/v1/orgs/{org}", { ...org, parseAs: "text" }).then(data)).toBe(JSON.stringify(builtinOwner));
    on("GET", "orgs/:org", new Response("{ not json", { status: 200, headers: { "content-type": "application/json" } }));
    expect(await api.GET("/platform/v1/orgs/{org}", { ...org, parseAs: "text" }).then(data)).toBe("{ not json");
    on("GET", "orgs/:org", new Response(JSON.stringify({ id: "org_1", name: "Northwind Labs" }), { status: 200 }));
    const plain = await api.GET("/platform/v1/orgs/{org}", org);
    expect(data(plain)).toEqual({ id: "org_1", name: "Northwind Labs" });
    on("GET", "orgs/:org", json({ id: "org_1", name: "Northwind Labs" }));
    const untouched = await api.GET("/platform/v1/orgs/{org}", org);
    expect(untouched.response.headers.get("content-type")).toBe("application/json");
    expect(data(untouched)).toEqual({ id: "org_1", name: "Northwind Labs" });
  });

  it("drop the original length once the body is rewritten", async () => {
    await speak("de");
    const body = JSON.stringify({ data: [builtinOwner] });
    on("GET", "orgs/:org/roles", new Response(body, { status: 200, headers: { "content-type": "application/json", "content-length": String(body.length) } }));
    const result = await api.GET("/platform/v1/orgs/{org}/roles", org);
    expect(result.response.headers.get("content-length")).toBeNull();
    expect(data(result).data[0]?.name).toBe("Inhaber");
  });
});
