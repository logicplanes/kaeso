import createClient, { type Middleware } from "openapi-fetch";
import { locale, t } from "~/i18n";
import type { paths } from "./generated/schema";
import { ApiError, errorFromResponse, networkError } from "./errors";

const throwOnError: Middleware = {
  async onResponse({ response }) {
    if (!response.ok) throw await errorFromResponse(response);
    return response;
  },
  onError({ error }) {
    if (error instanceof Error && error.name === "ApiError") return error;
    return networkError();
  },
};

const AS_LOGGED = /\/(logs|audit-log|playground)(\/|$)/;

const translateGatewayWords: Middleware = {
  async onResponse({ response, schemaPath }) {
    if (locale() === "en" || !response.ok || AS_LOGGED.test(schemaPath)) return undefined;
    if (!(response.headers.get("content-type") ?? "").includes("application/json")) return undefined;
    let body: unknown;
    try {
      const [{ translateBody }, parsed] = await Promise.all([import("./vocabulary"), response.clone().json() as Promise<unknown>]);
      if (!(await translateBody(parsed))) return undefined;
      body = parsed;
    } catch {
      return undefined;
    }
    const headers = new Headers(response.headers);
    headers.delete("content-length");
    return new Response(JSON.stringify(body), { status: response.status, statusText: response.statusText, headers });
  },
};

export const api = createClient<paths>({ baseUrl: "", credentials: "include" });
api.use(throwOnError, translateGatewayWords);

export function data<T>(result: { data?: T }): T {
  if (result.data === undefined)
    throw new ApiError({ type: "internal", status: 0, message: t("errors.emptyAnswer", "The gateway sent an empty answer. Try again.") });
  return result.data;
}
