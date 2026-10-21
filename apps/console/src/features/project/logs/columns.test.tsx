import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { DataTable } from "@logicplanes/kaeso-ui";
import { logRow } from "~/test/fixtures/project-logs";
import { COLUMNS, logColumns, logsLayout, type LogsLayout } from "./columns";

const shown = COLUMNS.filter((c) => c !== "request" && c !== "agent");

describe("logsLayout", () => {
  it("keeps the provider's name while the columns people compare fit, and its mark alone below that", () => {
    expect(logsLayout(0, false, true, shown)).toBe("wide");
    expect(logsLayout(1200, false, true, shown)).toBe("wide");
    expect(logsLayout(1000, false, true, shown)).toBe("medium");
    expect(logsLayout(1000, false, false, ["time", "status", "model", "provider"])).toBe("wide");
    expect(logsLayout(1000, false, true, ["duration"])).toBe("wide");
    expect(logsLayout(700, false, true, shown)).toBe("narrow");
    expect(logsLayout(1200, true, true, shown)).toBe("phone");
  });
});

function table(layout: LogsLayout, spansDays = true) {
  const all = logColumns({ spansDays, layout, keyName: (id) => id, partyName: (p) => p.id ?? "" });
  render(<DataTable aria-label="Requests" data={[logRow()]} columns={COLUMNS.map((c) => all[c])} getRowId={(r) => r.id} />);
  return screen.getAllByRole("cell");
}

describe("logColumns", () => {
  beforeEach(() => setFormatTimeZone("UTC"));

  it("names the time zone of the start time and shows the provider by name", () => {
    const cells = table("wide");
    expect(screen.getAllByRole("columnheader")[0]).toHaveTextContent("Started (UTC)");
    expect(cells[0]?.textContent).toMatch(/\d{2}:\d{2}:\d{2}$/);
    expect(screen.getByRole("columnheader", { name: /^Provider/ })).toBeInTheDocument();
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
  });

  it("shows the provider's mark alone, named by a tooltip, when the table is narrower", () => {
    table("medium");
    expect(screen.getByRole("columnheader", { name: /^Provider/ })).toBeInTheDocument();
    expect(screen.queryByText("OpenAI")).toBeNull();
    expect(screen.getByRole("img", { name: "OpenAI" })).toBeInTheDocument();
  });

  it("drops the seconds from a day's time on a tablet", () => {
    const [started] = table("narrow");
    expect(started?.textContent).toMatch(/\d{2}:\d{2}$/);
    expect(started?.textContent).not.toMatch(/:\d{2}:\d{2}/);
  });

  it("keeps the seconds within one hour on a phone", () => {
    expect(table("phone", false)[0]?.textContent).toMatch(/^\d{2}:\d{2}:\d{2}$/);
    expect(screen.getAllByRole("columnheader")[0]).toHaveTextContent(/^Started$/);
  });

  it("puts the model first in its cell, then the outcome, the source and the message", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "" });
    render(
      <DataTable
        aria-label="Requests"
        data={[logRow({ status: 499, error_kind: "client_disconnected", error_message: "The app closed the connection.", source: "playground" })]}
        columns={[all.model]}
        getRowId={(r) => r.id}
      />,
    );
    const parts = [...screen.getByRole("cell").querySelectorAll(".\\@container > *")].map((e) => e.textContent);
    expect(parts).toEqual(["gpt-5.4-mini", "Client disconnected", "Playground", "The app closed the connection."]);
  });

  it("starts with the outcome when a request was refused before a model was chosen", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "" });
    render(
      <DataTable
        aria-label="Requests"
        data={[logRow({ status: 400, model: "", provider: "", error_kind: "invalid_request" })]}
        columns={[all.model]}
        getRowId={(r) => r.id}
      />,
    );
    const parts = [...screen.getByRole("cell").querySelectorAll(".\\@container > *")].map((e) => e.textContent);
    expect(parts).toEqual(["Invalid request"]);
  });

  it("gives a successful Playground request's tag room sooner than a failed one's", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "" });
    render(
      <DataTable
        aria-label="Requests"
        data={[logRow({ source: "playground" }), logRow({ id: "req_0002", source: "playground", status: 429, error_kind: "rate_limited" })]}
        columns={[all.model]}
        getRowId={(r) => r.id}
      />,
    );
    const [ok, failed] = screen.getAllByText("Playground").map((e) => e.closest("[class*='@cell-']"));
    expect(ok).toHaveClass("@cell-sm:inline-flex");
    expect(failed).toHaveClass("@cell-lg:inline-flex");
  });
});
