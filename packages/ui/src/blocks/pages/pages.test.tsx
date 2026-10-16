import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { AuthPage, EmptyPage, ListPage, OverviewPage, TablePage } from "./pages.js";

describe("page blocks", () => {
  it("draws a list page with its heading, toolbar, rows and sheet", () => {
    render(
      <ListPage
        header={{ title: "API keys", actions: <button type="button">Create key</button> }}
        toolbar={<div role="toolbar" aria-label="Filters" />}
        sheet={<aside aria-label="Key details" />}
      >
        <ul aria-label="Keys">
          <li>production</li>
        </ul>
      </ListPage>,
    );
    expect(screen.getByRole("heading", { level: 1, name: "API keys" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create key" })).toBeInTheDocument();
    expect(screen.getByRole("toolbar", { name: "Filters" })).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "Keys" })).getByText("production")).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "Key details" })).toBeInTheDocument();
    expect(TablePage).toBe(ListPage);
  });

  it("draws an overview with metrics, controls, a chart, the breakdown and an aside", () => {
    const { rerender } = render(
      <OverviewPage
        header={{ title: "Overview" }}
        metrics={<p>1,204 requests</p>}
        controls={<button type="button">Last 7 days</button>}
        chart={<figure aria-label="Requests over time" />}
        aside={<p>Top models</p>}
      >
        <p>By project</p>
      </OverviewPage>,
    );
    expect(screen.getByRole("heading", { name: "Overview" })).toBeInTheDocument();
    expect(screen.getByText("1,204 requests")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Last 7 days" })).toBeInTheDocument();
    expect(screen.getByRole("figure", { name: "Requests over time" })).toBeInTheDocument();
    expect(screen.getByText("By project")).toBeInTheDocument();
    expect(within(screen.getByRole("complementary")).getByText("Top models")).toBeInTheDocument();
    rerender(<OverviewPage header={{ title: "Overview" }} metrics={<p>1,204 requests</p>} />);
    expect(screen.queryByRole("complementary")).toBeNull();
    expect(screen.queryByRole("button", { name: "Last 7 days" })).toBeNull();
  });

  it("centres an empty state, with or without a header", () => {
    const { rerender } = render(<EmptyPage header={{ title: "Projects" }} title="No projects yet" description="Create one to start routing requests." />);
    expect(screen.getByRole("heading", { name: "Projects" })).toBeInTheDocument();
    expect(screen.getByText("No projects yet")).toBeInTheDocument();
    expect(screen.getByText("Create one to start routing requests.")).toBeInTheDocument();
    rerender(<EmptyPage title="No projects yet" />);
    expect(screen.queryByRole("heading", { name: "Projects" })).toBeNull();
    expect(screen.getByText("No projects yet")).toBeInTheDocument();
  });

  it("draws a sign-in page with the wordmark, a heading and its form", () => {
    render(
      <AuthPage title="Sign in to Kaeso" description="Use your work email." footer={<span>Self-hosted at kaeso.example.com</span>}>
        <button type="button">Continue with SSO</button>
      </AuthPage>,
    );
    expect(screen.getByRole("heading", { level: 1, name: "Sign in to Kaeso" })).toBeInTheDocument();
    expect(screen.getByText("Use your work email.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Continue with SSO" })).toBeInTheDocument();
    expect(screen.getByText("Self-hosted at kaeso.example.com")).toBeInTheDocument();
  });
});
