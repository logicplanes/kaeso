import { Suspense } from "react";
import { act, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { lazyPending } from "./lazy-pending";

function Waiting() {
  return <p>Loading your session</p>;
}

describe("a pending component loaded on demand", () => {
  it("shows once its code arrives, after the Suspense fallback", async () => {
    let arrive = () => {};
    const load = vi.fn(() => new Promise<typeof Waiting>((resolve) => (arrive = () => resolve(Waiting))));
    const Pending = lazyPending(load);
    render(
      <Suspense fallback={<p>Opening</p>}>
        <Pending />
      </Suspense>,
    );
    expect(screen.getByText("Opening")).toBeInTheDocument();
    await act(async () => arrive());
    expect(await screen.findByText("Loading your session")).toBeInTheDocument();
    expect(screen.queryByText("Opening")).not.toBeInTheDocument();
  });

  it("preloads its code for the router, which then shows it", async () => {
    const load = vi.fn(() => Promise.resolve(Waiting));
    const Pending = lazyPending(load);
    await expect(Pending.preload()).resolves.toBeUndefined();
    expect(load).toHaveBeenCalledTimes(1);
    render(
      <Suspense fallback={<p>Opening</p>}>
        <Pending />
      </Suspense>,
    );
    expect(await screen.findByText("Loading your session")).toBeInTheDocument();
  });
});
