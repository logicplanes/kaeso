import { afterAll, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";

vi.mock("~/api/gateway-errors", () => {
  throw new Error("The translations didn't load.");
});
vi.stubGlobal(
  "fetch",
  async () =>
    new Response(
      `event: response.failed\ndata: ${JSON.stringify({ type: "response.failed", response: { error: { message: "Something went wrong; try again." } } })}\n\n`,
      {
        status: 200,
        headers: { "content-type": "text/event-stream" },
      },
    ),
);
const { apply, load } = await import("~/i18n");
const { useThreads } = await import("./use-threads");

afterAll(() => vi.unstubAllGlobals());

describe("useThreads without its translations", () => {
  it("shows the model's error as it came when the translations can't load", async () => {
    apply(await load("de"));
    const { result } = renderHook(() => useThreads("org_1", "proj_1", 1));
    act(() => result.current.send("Hi", [{ column: 0, request: { model: "gpt-5.4-mini", input: [], stream: true } }]));
    await waitFor(() => expect(result.current.threads[0]?.turns[0]?.response.error).toBe("Something went wrong; try again."));
  });
});
