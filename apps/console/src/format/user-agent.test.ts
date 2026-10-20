import { describe, expect, it } from "vitest";
import { describeUserAgent } from "./user-agent";

describe("describeUserAgent", () => {
  it("names common browsers and systems", () => {
    expect(describeUserAgent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36")).toBe(
      "Chrome on macOS",
    );
    expect(
      describeUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0"),
    ).toBe("Edge on Windows");
    expect(
      describeUserAgent(
        "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
      ),
    ).toBe("Safari on iOS");
    expect(describeUserAgent("Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0")).toBe("Firefox on Linux");
  });
  it("names the API clients scripts and SDKs send", () => {
    expect(describeUserAgent("node")).toBe("Node.js (API client)");
    expect(describeUserAgent("curl/8.7.1")).toBe("curl (API client)");
    expect(describeUserAgent("python-requests/2.32.3")).toBe("Python Requests (API client)");
    expect(describeUserAgent("OpenAI/Python 1.54.0")).toBe("OpenAI Python SDK (API client)");
    expect(describeUserAgent("Go-http-client/2.0")).toBe("Go (API client)");
  });
  it("keeps what it can't read, and says when there is nothing", () => {
    expect(describeUserAgent("checkout-worker/3.1")).toBe("checkout-worker/3.1");
    expect(describeUserAgent(null)).toBe("Unknown device");
  });
  it("names Opera, Android and ChromeOS", () => {
    expect(
      describeUserAgent("Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36 OPR/85.0.0.0"),
    ).toBe("Opera on Android");
    expect(describeUserAgent("Mozilla/5.0 (X11; CrOS x86_64 14541.0.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36")).toBe(
      "Chrome on ChromeOS",
    );
    expect(describeUserAgent("Mozilla/5.0 (Android 14; Mobile)")).toBe("Android");
  });
});
