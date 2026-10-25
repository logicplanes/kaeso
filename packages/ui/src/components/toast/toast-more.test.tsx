import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import { Toast, ToastCapsule, toast } from "./toast.js";

describe("toast tones and promises", () => {
  it("shows each tone with a description, and dismisses by id", async () => {
    render(<Toast limit={5} timeout={0} />);
    act(() => {
      toast.success("Key created", { description: "Copy it now.", id: "created" });
      toast.warning("Budget at 80%");
      toast.info("New models are available", { timeout: 0 });
    });
    expect(await screen.findByText("Key created")).toBeInTheDocument();
    expect(screen.getByText("Copy it now.", { exact: false })).toBeInTheDocument();
    expect(screen.getByText("Budget at 80%")).toBeInTheDocument();
    act(() => toast.dismiss("created"));
    await waitFor(() => expect(screen.queryByText("Key created")).toBeNull());
    act(() => Toast.add.dismiss());
  });

  it("keeps as many toasts in the stack as its limit allows", async () => {
    render(<Toast limit={5} timeout={0} />);
    act(() => {
      for (const name of ["production", "staging", "preview", "eval", "batch"]) toast(`Key ${name} rotated`);
    });
    expect(await screen.findByText("Key batch rotated")).toBeInTheDocument();
    for (const name of ["production", "staging", "preview", "eval", "batch"]) {
      const item = screen.getByText(`Key ${name} rotated`).closest(".k-toast");
      expect(item).not.toHaveAttribute("data-index-hidden");
      expect(item).not.toHaveAttribute("data-limited");
    }
    act(() => toast.dismiss());
  });

  it("follows a promise from loading to success or failure", async () => {
    render(<Toast />);
    let resolve: (v: string) => void = () => undefined;
    act(() => {
      void toast.promise(new Promise<string>((r) => (resolve = r)), { loading: "Saving…", success: (v) => `Saved ${v}`, error: "Couldn't save" });
    });
    expect((await screen.findAllByText("Saving…")).length).toBeGreaterThan(0);
    await act(async () => resolve("production"));
    expect((await screen.findAllByText("Saved production")).length).toBeGreaterThan(0);
    let reject: (e: Error) => void = () => undefined;
    act(() => {
      toast
        .promise(new Promise<string>((_, r) => (reject = r)), { loading: "Deleting…", success: "Deleted", error: (e) => `Failed: ${(e as Error).message}` })
        .catch(() => undefined);
    });
    await act(async () => reject(new Error("offline")));
    expect((await screen.findAllByText("Failed: offline")).length).toBeGreaterThan(0);
  });
});

describe("ToastCapsule", () => {
  it("draws a static toast for review pages", () => {
    render(<ToastCapsule tone="success" title="Key created" description="Copy it now." action="Undo" closable data-force="hover" />);
    expect(screen.getByRole("status")).toHaveTextContent("Key created Copy it now.");
    expect(screen.getByRole("button", { name: "Undo" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Dismiss" })).toHaveAttribute("data-force", "visible hover");
    render(<ToastCapsule title="Plain" />);
    expect(screen.getByText("Plain")).toBeInTheDocument();
  });

  it("forwards its ref to the status element", () => {
    const ref = createRef<HTMLDivElement>();
    render(<ToastCapsule ref={ref} title="Key revoked" />);
    expect(ref.current).toBe(screen.getByRole("status"));
  });
});
