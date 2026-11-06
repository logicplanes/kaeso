import { type ComponentType, Suspense, useState } from "react";
import { lazyRouteComponent } from "@tanstack/react-router";
import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

function Page() {
  return <p>Sessions</p>;
}

function Host({ Route }: { Route: ComponentType }) {
  const [count, setCount] = useState(0);
  return (
    <Suspense fallback={<p>Opening</p>}>
      <button type="button" onClick={() => setCount((c) => c + 1)}>
        Again {count}
      </button>
      <Route />
    </Suspense>
  );
}

describe("a code-split route component", () => {
  afterEach(() => vi.restoreAllMocks());

  it("renders again after its module arrived without React reporting a use() that went missing", async () => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    let arrive = () => {};
    const Route = lazyRouteComponent(() => new Promise<{ default: typeof Page }>((resolve) => (arrive = () => resolve({ default: Page }))));
    await act(async () => {
      render(<Host Route={Route} />);
    });
    expect(screen.getByText("Opening")).toBeInTheDocument();
    await act(async () => arrive());
    expect(errors.mock.calls.flat().join("\n")).not.toMatch(/conditional-use-of-use/);
    expect(await screen.findByText("Sessions")).toBeInTheDocument();
    await act(async () => screen.getByRole("button", { name: /Again/ }).click());
    expect(screen.getByText("Again 1")).toBeInTheDocument();
    expect(screen.getByText("Sessions")).toBeInTheDocument();
    expect(errors.mock.calls.flat().join("\n")).not.toMatch(/conditional-use-of-use/);
  });

  it("renders at once when its module was preloaded, and again after a state change", async () => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    const Route = lazyRouteComponent(() => Promise.resolve({ default: Page }));
    await Route.preload?.();
    render(<Host Route={Route} />);
    expect(await screen.findByText("Sessions")).toBeInTheDocument();
    await act(async () => screen.getByRole("button", { name: /Again/ }).click());
    expect(screen.getByText("Again 1")).toBeInTheDocument();
    expect(errors.mock.calls.flat().join("\n")).not.toMatch(/conditional-use-of-use/);
  });
});
