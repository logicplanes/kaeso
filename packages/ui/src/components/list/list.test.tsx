import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { List, SelectionAction } from "./list.js";
import { InsetContent } from "../../utils/inset.js";
import { Empty } from "../empty/empty.js";
import { Dialog } from "../dialog/dialog.js";
import { Menu } from "../menu/menu.js";
import { IconButton } from "../icon-button/icon-button.js";
import { icons } from "../../icons/vocabulary.js";

const items = ["checkout-web", "checkout-worker", "ci-runner"].map((n) => ({ id: n, title: n }));

describe("List", () => {
  it("is a raised card in a padded column", () => {
    const { rerender } = render(<List aria-label="Keys" items={items} />);
    expect(screen.getByRole("grid")).not.toHaveClass("rounded-group");
    rerender(
      <InsetContent>
        <List aria-label="Keys" items={items} />
      </InsetContent>,
    );
    expect(screen.getByRole("grid")).toHaveClass("rounded-group");
  });
  it("puts a leading avatar before the title", () => {
    render(<List aria-label="Members" items={[{ id: "ada", title: "Ada", leading: <span data-testid="avatar" /> }]} />);
    const cell = screen.getByRole("gridcell");
    expect(within(cell).getByTestId("avatar").compareDocumentPosition(within(cell).getByText("Ada")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
  it("is a grid of one-cell rows", () => {
    render(<List aria-label="Keys" items={items} />);
    expect(within(screen.getByRole("grid", { name: "Keys" })).getAllByRole("gridcell")).toHaveLength(3);
  });
  it("selects with x and shows the selection bar with its count", async () => {
    render(<List aria-label="Keys" items={items} selectable selectionActions={<SelectionAction>Disable</SelectionAction>} />);
    screen.getAllByRole("gridcell")[0]!.focus();
    await userEvent.keyboard("j");
    await userEvent.keyboard("x");
    expect(screen.getAllByRole("gridcell")[1]).toHaveAttribute("aria-selected", "true");
    const bar = screen.getByRole("region", { name: "Selection" });
    expect(bar).toHaveTextContent("1 selected");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Selection" })).not.toBeInTheDocument();
  });
  it("collapses a group from its header", async () => {
    render(<List aria-label="Keys" groups={[{ id: "active", title: "Active", items }]} />);
    await userEvent.click(screen.getByRole("button", { name: /Active/ }));
    expect(screen.queryAllByRole("gridcell").filter((c) => c.hasAttribute("data-row"))).toHaveLength(0);
  });
  it("keeps the selection without a floating bar when the host shows it", async () => {
    render(<List aria-label="Models" items={items} selectable selectionBar={false} />);
    await userEvent.click(screen.getByRole("checkbox", { name: "Select checkout-web" }));
    expect(screen.getAllByRole("gridcell")[0]).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("region", { name: "Selection" })).not.toBeInTheDocument();
  });
  it("shows loading rows, busy, until rows arrive", () => {
    const { rerender } = render(<List aria-label="Keys" items={[]} loading />);
    expect(screen.getByRole("grid", { name: "Keys" })).toHaveAttribute("aria-busy", "true");
    expect(screen.queryAllByRole("gridcell")).toHaveLength(0);
    rerender(<List aria-label="Keys" items={items} loading={false} />);
    expect(screen.getByRole("grid", { name: "Keys" })).not.toHaveAttribute("aria-busy");
    expect(screen.getAllByRole("gridcell")).toHaveLength(3);
  });
  it("shows its empty state when there are no rows, inside the card in a padded column", () => {
    const empty = <Empty size="md" title="No invitations yet" />;
    const { rerender } = render(<List aria-label="Invitations" items={[]} empty={empty} />);
    const grid = screen.getByRole("grid", { name: "Invitations" });
    expect(within(grid).getByRole("heading", { name: "No invitations yet" })).toBeInTheDocument();
    expect(grid).not.toHaveClass("rounded-group");
    rerender(
      <InsetContent>
        <List aria-label="Invitations" items={[]} empty={empty} />
      </InsetContent>,
    );
    expect(screen.getByRole("grid", { name: "Invitations" })).toHaveClass("rounded-group");
    expect(within(screen.getByRole("grid", { name: "Invitations" })).getByRole("heading", { name: "No invitations yet" })).toBeInTheDocument();
  });
  it("shows loading rows, not the empty state, while loading", () => {
    render(<List aria-label="Invitations" items={[]} loading empty={<Empty title="No invitations yet" />} />);
    expect(screen.queryByRole("heading", { name: "No invitations yet" })).not.toBeInTheDocument();
  });
  it("shows the empty state when every group is empty, and rows otherwise", () => {
    const { rerender } = render(<List aria-label="Keys" groups={[{ id: "active", title: "Active", items: [] }]} empty={<Empty title="No keys" />} />);
    expect(screen.getByRole("heading", { name: "No keys" })).toBeInTheDocument();
    rerender(<List aria-label="Keys" items={items} empty={<Empty title="No keys" />} />);
    expect(screen.queryByRole("heading", { name: "No keys" })).not.toBeInTheDocument();
  });
  it("caps both lines of a stacked row at the column's width so long titles truncate", () => {
    render(
      <InsetContent>
        <List
          aria-label="Members"
          items={[
            { id: "ada", title: "Ada Okafor-Lindqvist from the platform team", subtitle: "ada.okafor-lindqvist@northwind.example", meta: <span>Owner</span> },
          ]}
        />
      </InsetContent>,
    );
    expect(screen.getByText("Ada Okafor-Lindqvist from the platform team")).toHaveClass("max-w-full", "truncate");
    expect(screen.getByText("ada.okafor-lindqvist@northwind.example")).toHaveClass("max-w-full", "truncate");
  });
  it("gives controls in a selected row the on-fill material", () => {
    render(<List aria-label="Keys" items={items} selectable selected={new Set(["checkout-web"])} />);
    expect(screen.getAllByRole("gridcell")[0]).toHaveClass("controls-on-fill");
    expect(screen.getAllByRole("gridcell")[1]).not.toHaveClass("controls-on-fill");
  });
  it("is not a card in a dialog opened from a padded column", async () => {
    render(
      <InsetContent>
        <Dialog defaultOpen title="Add members">
          <List aria-label="People" items={items} />
        </Dialog>
      </InsetContent>,
    );
    expect(await screen.findByRole("grid", { name: "People" })).not.toHaveClass("rounded-group");
  });
  it("loads no checkbox for a list that isn't selectable", () => {
    render(<List aria-label="Keys" items={items} />);
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });
  it("opens no row from a menu its actions opened, by click or by Enter", async () => {
    const opened: string[] = [];
    const chosen: string[] = [];
    render(
      <List
        aria-label="Projects"
        onOpen={(item) => opened.push(item.id)}
        items={items.map((i) => ({
          ...i,
          actions: (
            <Menu>
              <Menu.Trigger>
                <IconButton icon={icons.more} aria-label={`Actions for ${i.id}`} size="xs" />
              </Menu.Trigger>
              <Menu.Content>
                <Menu.Item onClick={() => chosen.push(i.id)}>Make default</Menu.Item>
              </Menu.Content>
            </Menu>
          ),
        }))}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Actions for ci-runner" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Make default" }));
    await userEvent.click(screen.getByRole("button", { name: "Actions for checkout-web" }));
    (await screen.findByRole("menuitem", { name: "Make default" })).focus();
    await userEvent.keyboard("{Enter}");
    expect(chosen).toEqual(["ci-runner", "checkout-web"]);
    expect(opened).toEqual([]);
    await userEvent.click(screen.getByText("checkout-worker"));
    expect(opened).toEqual(["checkout-worker"]);
  });
  it("narrow (a phone), puts a row's metadata and date on a line under its title, and grows to hold them", () => {
    const rows = [{ id: "a", title: "shell-invitee@example.com", subtitle: "Invited by Ada", meta: <span>Accepted</span>, date: "Sent 13 Oct 2026" }];
    const spy = vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(360);
    const { unmount } = render(<List aria-label="Invitations" items={rows} />);
    const cell = screen.getByRole("gridcell");
    expect(cell).toHaveClass("min-h-56");
    const column = within(cell).getByText("Sent 13 Oct 2026").closest(".flex-col");
    expect(column).toContainElement(within(cell).getByText("shell-invitee@example.com"));
    expect(column).toContainElement(within(cell).getByText("Accepted"));
    unmount();
    spy.mockReturnValue(900);
    render(<List aria-label="Invitations" items={rows} />);
    expect(screen.getByRole("gridcell")).not.toHaveClass("min-h-56");
    spy.mockRestore();
  });
});
