import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { signedIn } from "~/test/fixtures";
import { catalogueModel, projectData, projectPath } from "~/test/fixtures/project";
import { logRow, logsData } from "~/test/fixtures/project-logs";
import { renderApp } from "~/test/render";

const built = vi.hoisted(() => ({ sizes: [] as number[] }));

vi.mock("@logicplanes/kaeso-ui/playground", async (importOriginal) => {
  const ui = await importOriginal<typeof import("@logicplanes/kaeso-ui/playground")>();
  return {
    ...ui,
    useModelGroups: (...args: Parameters<typeof ui.useModelGroups>) => {
      built.sizes.push(args[0].length);
      return ui.useModelGroups(...args);
    },
  };
});

describe("The request log's Model filter", () => {
  it("builds the catalogue's model choices only once the filter is added", async () => {
    signedIn();
    const models = Array.from({ length: 30 }, (_, i) =>
      catalogueModel({
        id: `cm_${i}`,
        provider: i % 2 ? "anthropic" : "openai",
        model_id: `model-${i}`,
        effective: { ...catalogueModel().effective, display_name: null },
      }),
    );
    projectData({ models });
    logsData({ rows: [logRow()] });
    const { user, router } = await renderApp(projectPath("logs"));

    await screen.findByRole("row", { name: /key_web/ });
    expect(built.sizes).toEqual([]);

    await user.click(screen.getByRole("button", { name: "Add filter" }));
    await user.click(within(await screen.findByRole("menu", { name: "Add filter" })).getByRole("menuitem", { name: "Model" }));
    const list = await screen.findByRole("listbox", { name: "Model" });
    expect(built.sizes.at(-1)).toBe(30);
    expect(within(list).getByRole("group", { name: "Anthropic" })).toBeInTheDocument();
    await user.type(screen.getByRole("combobox", { name: "Search model values" }), "model-29");
    await user.click(within(list).getByRole("option", { name: /^model-29/ }));
    await waitFor(() => expect(router.state.location.search).toEqual({ model: "model-29" }));
    await waitFor(() => expect(built.sizes.at(-1)).toBe(30));
  });
});
