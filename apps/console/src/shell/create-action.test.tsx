import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";
import { CreateActionProvider, useCreateAction, usePageCreateAction } from "./create-action";

function Offered() {
  const action = useCreateAction();
  return <p>{action ? action.label : "Nothing to create"}</p>;
}

function Page() {
  usePageCreateAction({ label: "Create key", run: () => undefined });
  return <Offered />;
}

describe("a page's create action", () => {
  it("is offered inside the console frame", async () => {
    render(
      <CreateActionProvider>
        <Page />
      </CreateActionProvider>,
    );
    expect(await screen.findByText("Create key")).toBeInTheDocument();
  });

  it("is ignored outside the console frame", () => {
    render(<Page />);
    expect(screen.getByText("Nothing to create")).toBeInTheDocument();
  });

  it("runs the page's newest handler, and the page doesn't draw again when it is offered", async () => {
    let pageDraws = 0;
    const runs: number[] = [];
    function Counting() {
      const [n, setN] = useState(0);
      pageDraws++;
      usePageCreateAction({ label: "Create key", run: () => runs.push(n) });
      return <button onClick={() => setN((x) => x + 1)}>Count {n}</button>;
    }
    function Offer() {
      const action = useCreateAction();
      return action ? <button onClick={action.run}>{action.label}</button> : null;
    }
    const user = userEvent.setup();
    render(
      <CreateActionProvider>
        <Counting />
        <Offer />
      </CreateActionProvider>,
    );
    await user.click(await screen.findByRole("button", { name: "Create key" }));
    expect(pageDraws).toBe(1);
    await user.click(screen.getByRole("button", { name: "Count 0" }));
    await user.click(screen.getByRole("button", { name: "Create key" }));
    expect(runs).toEqual([0, 1]);
    expect(pageDraws).toBe(2);
  });
});
