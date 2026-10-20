import { t } from "~/i18n";

const CLIENTS: readonly (readonly [RegExp, string])[] = [
  [/^OpenAI\/Python\b/i, "OpenAI Python SDK"],
  [/^OpenAI\/JS\b/i, "OpenAI Node.js SDK"],
  [/^Anthropic\/Python\b/i, "Anthropic Python SDK"],
  [/^Anthropic\/JS\b/i, "Anthropic Node.js SDK"],
  [/^(node|undici|node-fetch)\b/i, "Node.js"],
  [/^Bun\//i, "Bun"],
  [/^Deno\//i, "Deno"],
  [/^axios\//i, "axios"],
  [/^curl\//i, "curl"],
  [/^Wget\//i, "Wget"],
  [/^python-requests\//i, "Python Requests"],
  [/^python-httpx\//i, "Python HTTPX"],
  [/^(python-urllib|Python-urllib)\//i, "Python"],
  [/^aiohttp\//i, "Python aiohttp"],
  [/^Go-http-client\//i, "Go"],
  [/^okhttp\//i, "OkHttp"],
  [/^PostmanRuntime\//i, "Postman"],
  [/^insomnia\//i, "Insomnia"],
];

function apiClient(ua: string): string | null {
  const name = CLIENTS.find(([pattern]) => pattern.test(ua))?.[1];
  return name ? t("format.userAgent.apiClient", "{client} (API client)", { client: name }) : null;
}

export function describeUserAgent(ua: string | null | undefined): string {
  if (!ua) return t("format.userAgent.unknown", "Unknown device");
  const client = apiClient(ua);
  if (client) return client;
  const browser = /Edg\//.test(ua)
    ? "Edge"
    : /OPR\/|Opera/.test(ua)
      ? "Opera"
      : /Firefox\//.test(ua)
        ? "Firefox"
        : /Chrome\/|CriOS\//.test(ua)
          ? "Chrome"
          : /Safari\//.test(ua) && /Version\//.test(ua)
            ? "Safari"
            : null;
  const os = /iPhone|iPad|iPod/.test(ua)
    ? "iOS"
    : /Android/.test(ua)
      ? "Android"
      : /Mac OS X|Macintosh/.test(ua)
        ? "macOS"
        : /Windows/.test(ua)
          ? "Windows"
          : /CrOS/.test(ua)
            ? "ChromeOS"
            : /Linux/.test(ua)
              ? "Linux"
              : null;
  if (browser && os) return t("format.userAgent.browserOnOs", "{browser} on {os}", { browser, os });
  return browser ?? os ?? ua;
}
