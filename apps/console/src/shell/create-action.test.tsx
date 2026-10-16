import { render, screen } from "@testing-library/react";
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
});
