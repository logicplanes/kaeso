import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ResourceList } from "./resource-list";
import type { ListGroup } from "../../components/list/list";
import { icons } from "../../icons/vocabulary";

const groups: ListGroup[] = [
  {
    id: "active",
    title: "Active",
    items: [
      { id: "k1", title: "production", displayId: "sk-kaeso-1a2b" },
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
});
