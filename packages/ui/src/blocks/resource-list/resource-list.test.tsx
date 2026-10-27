import { createRef } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ResourceList } from "./resource-list.js";
import type { ListGroup } from "../../components/list/list.js";
import { icons } from "../../icons/vocabulary.js";

const groups: ListGroup[] = [
  {
    id: "active",
    title: "Active",
    items: [
      { id: "k1", title: "production", displayId: "sk-kso-1a2b" },
      { id: "k2", title: "staging", textValue: "staging key" },
    ],
  },
];

describe("ResourceList", () => {
  it("searches titles and ids, and says when nothing matches", async () => {
    render(<ResourceList aria-label="Keys" groups={groups} />);
    const search = screen.getByRole("searchbox", { name: "Search" });
    await userEvent.type(search, "1a2b");
    expect(screen.getByText("production")).toBeInTheDocument();
    expect(screen.queryByText("staging")).toBeNull();
    await userEvent.clear(search);
    await userEvent.type(search, "zzz");
    expect(screen.getByText("No matches")).toBeInTheDocument();
    expect(screen.getByText("Try a shorter search.")).toBeInTheDocument();
  });

  it("shows its empty state with an action", () => {
    render(
      <ResourceList
        aria-label="Keys"
        groups={[]}
        emptyTitle="No keys yet"
        emptyDescription="Create one to call the gateway."
        emptyAction={<button>Create key</button>}
        toolbar={<span>Filters here</span>}
      />,
    );
    expect(screen.getByText("No keys yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Create key" })).toBeInTheDocument();
    expect(screen.getByText("Filters here")).toBeInTheDocument();
  });

  it("offers bulk actions for the selected rows and opens a row", async () => {
    const onArchive = vi.fn();
    const onOpen = vi.fn();
    render(
      <ResourceList
        aria-label="Keys"
        groups={groups}
        onOpen={onOpen}
        bulkActions={[
          { label: "Archive", onClick: onArchive },
          { label: "Export", icon: icons.download, onClick: vi.fn() },
        ]}
      />,
    );
    fireEvent.click(screen.getByRole("checkbox", { name: "Select production" }));
    await userEvent.click(await screen.findByRole("button", { name: "Archive" }));
    expect(onArchive).toHaveBeenCalledWith(["k1"]);
  });

  it("acts only on the selected rows that are still shown", async () => {
    const onArchive = vi.fn();
    const bulk = [{ label: "Archive", onClick: onArchive }];
    const { rerender } = render(<ResourceList aria-label="Keys" groups={groups} bulkActions={bulk} />);
    fireEvent.click(screen.getByRole("checkbox", { name: "Select production" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select staging key" }));
    expect(await screen.findByText("2 selected")).toBeInTheDocument();
    await userEvent.type(screen.getByRole("searchbox", { name: "Search" }), "staging");
    expect(screen.getByText("1 selected")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Archive" }));
    expect(onArchive).toHaveBeenLastCalledWith(["k2"]);
    await userEvent.clear(screen.getByRole("searchbox", { name: "Search" }));
    rerender(<ResourceList aria-label="Keys" groups={groups.map((g) => ({ ...g, items: g.items.filter((i) => i.id === "k1") }))} bulkActions={bulk} />);
    expect(screen.getByText("1 selected")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Archive" }));
    expect(onArchive).toHaveBeenLastCalledWith(["k1"]);
  });
});

describe("ResourceList's ref", () => {
  it("reaches the element around the toolbar and the list", () => {
    const ref = createRef<HTMLDivElement>();
    render(<ResourceList ref={ref} aria-label="Keys" groups={groups} />);
    expect(ref.current).toContainElement(screen.getByRole("toolbar", { name: "Filters" }));
    expect(ref.current).toContainElement(screen.getByText("production"));
  });

  it("names the collection when it is empty without a title of its own", () => {
    render(<ResourceList aria-label="Keys" groups={[]} />);
    expect(screen.getByText("No keys yet")).toBeInTheDocument();
  });

  it("announces how many rows a search leaves, and clears the search from its no-match state", async () => {
    render(<ResourceList aria-label="Keys" groups={groups} />);
    const status = screen.getByRole("status");
    expect(status).toHaveTextContent("");
    await userEvent.type(screen.getByRole("searchbox", { name: "Search" }), "prod");
    expect(status).toHaveTextContent("1 result");
    await userEvent.type(screen.getByRole("searchbox", { name: "Search" }), "zzz");
    expect(status).toHaveTextContent("No results");
    await userEvent.click(screen.getAllByRole("button", { name: "Clear search" }).at(-1)!);
    expect(screen.getByRole("searchbox", { name: "Search" })).toHaveValue("");
    expect(screen.getByText("staging")).toBeInTheDocument();
  });
});
