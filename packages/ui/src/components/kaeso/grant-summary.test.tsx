import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { GrantSummary } from "./grant-summary.js";

describe("GrantSummary", () => {
  it("lists every line under its name, with the note below", () => {
    render(<GrantSummary aria-label="What it can do" items={["Manages projects", "Reads the rest"]} footer="For 90 days" />);
    const list = screen.getByRole("list", { name: "What it can do" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["Manages projects", "Reads the rest"]);
    expect(screen.getByText("For 90 days")).toBeInTheDocument();
  });

  it("leaves the note out when there is none", () => {
    const { container } = render(<GrantSummary aria-label="Rights" items={["Reads the instance"]} />);
    expect(container.querySelector("p")).toBeNull();
  });

  it("puts an action next to the note", () => {
    render(<GrantSummary aria-label="Rights" items={["Reads the instance"]} footer="For 7 days" action={<button type="button">Customize</button>} />);
    expect(screen.getByRole("button", { name: "Customize" })).toBeInTheDocument();
    expect(screen.getByText("For 7 days")).toBeInTheDocument();
  });
  it("with sections, lists one collapsible row per server with its count, and shows the names as code once opened", async () => {
    render(
      <GrantSummary
        aria-label="Tools it can call"
        sections={[
          { id: "mcp_x", title: "X", hint: "2 tools", items: ["x__create_post", "x__search_posts"] },
          { id: "mcp_linear", title: "Linear", hint: "1 tool", items: ["linear__get_issue"], defaultOpen: true },
        ]}
        footer="3 tools in Northwind › Checkout"
      />,
    );
    const list = screen.getByRole("list", { name: "Tools it can call" });
    expect([...list.querySelectorAll(":scope > li")].map((li) => li.textContent)).toEqual(["X2 tools", "Linear1 toollinear__get_issue"]);
    expect(screen.queryByText("x__create_post")).toBeNull();
    await userEvent.click(within(list).getByRole("button", { name: /^X/ }));
    expect(await screen.findByText("x__create_post")).toBeInTheDocument();
    expect(screen.getByText("x__search_posts").closest("code")).not.toBeNull();
    expect(screen.getByText("3 tools in Northwind › Checkout")).toBeInTheDocument();
  });
});
