import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Table } from "./table.js";

describe("Table", () => {
  it("leaves room at the end of a text column for a link's hover bleed, and keeps numbers on the edge", () => {
    render(
      <Table>
        <Table.Head>
          <Table.Row>
            <Table.HeaderCell>Model</Table.HeaderCell>
            <Table.HeaderCell>Notes</Table.HeaderCell>
            <Table.HeaderCell align="end">Requests</Table.HeaderCell>
          </Table.Row>
        </Table.Head>
        <Table.Body>
          <Table.Row>
            <Table.Cell>claude-haiku-4-5</Table.Cell>
            <Table.Cell>
              <a href="/console/models-and-aliases">Aliases</a>
            </Table.Cell>
            <Table.Cell align="end">1,200</Table.Cell>
          </Table.Row>
        </Table.Body>
      </Table>,
    );
    for (const name of ["Model", "Notes"]) expect(screen.getByRole("columnheader", { name })).toHaveClass("first:pl-0", "last:pr-2");
    expect(screen.getByRole("columnheader", { name: "Requests" })).toHaveClass("last:pr-0", "text-right");
    expect(screen.getByRole("cell", { name: "Aliases" })).toHaveClass("last:pr-2");
    expect(screen.getByRole("cell", { name: "1,200" })).toHaveClass("last:pr-0", "tabular");
  });

  it("keeps a header on one line, or lets a long one wrap at the foot of its row", () => {
    render(
      <Table>
        <Table.Head>
          <Table.Row>
            <Table.HeaderCell>Model</Table.HeaderCell>
            <Table.HeaderCell wrap>Through another organization's identity provider</Table.HeaderCell>
          </Table.Row>
        </Table.Head>
      </Table>,
    );
    expect(screen.getByRole("columnheader", { name: "Model" })).toHaveClass("whitespace-nowrap", "h-36");
    const long = screen.getByRole("columnheader", { name: /another organization/ });
    expect(long).not.toHaveClass("whitespace-nowrap");
    expect(long).toHaveClass("py-8", "align-bottom", "h-36");
  });

  it("puts 12px between columns, or 8px in a compact table", () => {
    const table = (density?: "compact") => (
      <Table density={density}>
        <Table.Head>
          <Table.Row>
            <Table.HeaderCell>Variable</Table.HeaderCell>
          </Table.Row>
        </Table.Head>
        <Table.Body>
          <Table.Row>
            <Table.Cell>KAESO_LISTEN</Table.Cell>
          </Table.Row>
        </Table.Body>
      </Table>
    );
    const { rerender } = render(table());
    expect(screen.getByRole("columnheader")).toHaveClass("px-12", "first:pl-0");
    expect(screen.getByRole("cell")).toHaveClass("px-12", "first:pl-0");
    rerender(table("compact"));
    expect(screen.getByRole("columnheader")).toHaveClass("px-8", "first:pl-0");
    expect(screen.getByRole("cell")).toHaveClass("px-8", "first:pl-0");
    expect(screen.getByRole("cell")).not.toHaveClass("px-12");
  });
});
