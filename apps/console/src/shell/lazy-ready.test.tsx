import { lazy, Suspense } from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { lazyReady } from "./lazy-ready";

function Banner({ title }: { title: string }) {
  return <p>{title}</p>;
}

const page = (part: React.ReactNode) => <Suspense fallback={<p>Loading</p>}>{part}</Suspense>;

describe("a part split from its page", () => {
  it("draws with the page once the route preloaded its code, where a plain lazy part shows its fallback first", async () => {
    const Ready = lazyReady(() => Promise.resolve(Banner));
    const Plain = lazy(() => Promise.resolve({ default: Banner }));
    await Ready.preload();

    const ready = render(page(<Ready title="2 projects use retiring models" />));
    expect(screen.getByText("2 projects use retiring models")).toBeInTheDocument();
    expect(screen.queryByText("Loading")).not.toBeInTheDocument();
    ready.unmount();

    render(page(<Plain title="2 projects use retiring models" />));
    expect(screen.getByText("Loading")).toBeInTheDocument();
    expect(await screen.findByText("2 projects use retiring models")).toBeInTheDocument();
  });

  it("loads on its own when nothing preloaded it, and loads once", async () => {
    let loads = 0;
    const Ready = lazyReady(() => {
      loads += 1;
      return Promise.resolve(Banner);
    });
    render(page(<Ready title="Kaeso 40.4.2" />));
    expect(screen.getByText("Loading")).toBeInTheDocument();
    expect(await screen.findByText("Kaeso 40.4.2")).toBeInTheDocument();
    await Ready.preload();
    expect(loads).toBe(1);
  });

  it("tries again after its code failed to load", async () => {
    let loads = 0;
    const Ready = lazyReady(() => (++loads === 1 ? Promise.reject(new Error("offline")) : Promise.resolve(Banner)));
    await expect(Ready.preload()).rejects.toThrow("offline");
    await Ready.preload();
    render(page(<Ready title="Kaeso 40.4.2" />));
    expect(screen.getByText("Kaeso 40.4.2")).toBeInTheDocument();
    expect(loads).toBe(2);
  });
});
