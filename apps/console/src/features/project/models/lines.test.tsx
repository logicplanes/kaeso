import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { catalogueModel, modelsApi, preview, projectData, projectPath, searchHit } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import type { ModelLineView } from "~/api/generated/types";

const LINE = "model_line";
const LINES = "orgs/:org/projects/:project/lines";

const line = (over: Partial<ModelLineView> = {}): ModelLineView => ({
  id: LINE,
  provider: "openai",
  line: "gpt-latest",
  model: "gpt-5.5",
  previous: "gpt-5.4",
  since: "2026-10-01T00:00:00Z",
  policy: "follow",
  delay_days: null,
  kpmr_model: "gpt-5.5",
  pending: null,
  kept: null,
  held: null,
  ...over,
});

function setUp(lines: ModelLineView[], rights?: string[]) {
  signedIn(rights ? { projectRights: rights } : {});
  projectData();
  const entry = catalogueModel({
    id: LINE,
    provider: "openai",
    model_id: "gpt-latest",
    line_target: "gpt-5.5",
    effective: { ...catalogueModel().effective, display_name: "gpt-latest" },
  });
  modelsApi({ models: [entry], hits: [searchHit()], previews: [preview()] });
  on("GET", LINES, { data: lines });
}

describe("model lines", () => {
  it("are offered in Add models and added with a policy", async () => {
    setUp([]);
    on("GET", `${LINES}/offered`, {
      data: [
        { provider: "openai", line: "gpt-latest", model: "gpt-5.5", since: "2026-09-01", previous: "gpt-5.4", added: "model_line" },
        { provider: "openai", line: "gpt-mini-latest", model: "gpt-5.4-mini", since: null, previous: null, added: null },
      ],
    });
    on("POST", LINES, line({ line: "gpt-mini-latest", model: "gpt-5.4-mini", policy: "delay", delay_days: 3 }));
    const { user } = await renderApp(projectPath("models?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(await within(dialog).findByRole("button", { name: /Add a .* model line/ }));
    expect(await screen.findByText("gpt-mini-latest")).toBeInTheDocument();
    expect(screen.getByText("In the catalogue")).toBeInTheDocument();
    await user.click(screen.getByRole("combobox", { name: "When KPMR moves the line" }));
    await user.click(await screen.findByRole("option", { name: /Delay/ }));
    const days = screen.getByRole("spinbutton", { name: "Days to wait" });
    await user.clear(days);
    expect(days).toHaveValue(1);
    await user.tripleClick(days);
    await user.keyboard("3");
    await user.click(screen.getByRole("button", { name: "Add line" }));
    expect(await notifications().findByText("gpt-mini-latest added; it points at gpt-5.4-mini")).toBeInTheDocument();
    expect(requests("POST", LINES).map((r) => r.body)).toEqual([{ provider: "openai", line: "gpt-mini-latest", policy: "delay", delay_days: 3 }]);
  });

  it("show the line being added as busy", async () => {
    setUp([]);
    on("GET", `${LINES}/offered`, {
      data: [
        { provider: "openai", line: "gpt-mini-latest", model: "gpt-5.4-mini", since: null, previous: null, added: null },
        { provider: "openai", line: "gpt-nano-latest", model: "gpt-5.4-nano", since: null, previous: null, added: null },
      ],
    });
    on("POST", LINES, () => new Promise(() => undefined));
    const { user } = await renderApp(projectPath("models?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(await within(dialog).findByRole("button", { name: /Add a .* model line/ }));
    const buttons = await screen.findAllByRole("button", { name: "Add line" });
    expect(buttons).toHaveLength(2);
    const [mini, nano] = buttons as [HTMLElement, HTMLElement];
    await user.click(mini);
    await waitFor(() => expect(mini).toHaveAttribute("aria-busy", "true"));
    expect(nano).not.toHaveAttribute("aria-busy");
  });

  it("say when a provider has none", async () => {
    setUp([]);
    on("GET", `${LINES}/offered`, { data: [] });
    const { user } = await renderApp(projectPath("models?add=true"));
    const dialog = await screen.findByRole("dialog", { name: /Add models/ });
    await user.click(await within(dialog).findByRole("button", { name: /Add a .* model line/ }));
    expect(await screen.findByText(/has no model lines/)).toBeInTheDocument();
  });

  it("show where a line points in the catalogue and its sheet, and change its policy", async () => {
    setUp([line()]);
    on("PATCH", `${LINES}/:model`, line({ policy: "pin" }));
    on("POST", `${LINES}/:model/back`, line({ model: "gpt-5.4", previous: "gpt-5.5", policy: "pin" }));
    const { user } = await renderApp(projectPath(`models?model=${LINE}`));
    expect(await screen.findByText("Line → gpt-5.5")).toBeInTheDocument();
    const sheet = await screen.findByRole("dialog", { name: /gpt-latest/ });
    expect(await within(sheet).findByText("Points at")).toBeInTheDocument();
    expect(within(sheet).getByText("gpt-5.4")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("combobox", { name: "When KPMR moves the line" }));
    await user.click(await screen.findByRole("option", { name: /Pin/ }));
    expect(await notifications().findByText("Line settings saved")).toBeInTheDocument();
    expect(requests("PATCH", `${LINES}/:model`).map((r) => r.body)).toEqual([{ policy: "pin" }]);
    await user.click(within(sheet).getByRole("button", { name: "Move back to gpt-5.4" }));
    expect(await notifications().findByText("gpt-latest is back on gpt-5.4 and pinned")).toBeInTheDocument();
  });

  it("offer to move or keep a move that waits, and say why one is held", async () => {
    setUp([
      line({
        policy: "ask",
        previous: null,
        kpmr_model: "gpt-6",
        pending: { model: "gpt-6", since: "2026-10-14T00:00:00Z", moves_at: null },
        held: "No provider key for `openai` is set up for this project, so the line stays on `gpt-5.5`.",
      }),
    ]);
    on("POST", `${LINES}/:model/move`, line({ model: "gpt-6" }));
    on("POST", `${LINES}/:model/keep`, apiError(409, "conflict", "No move waits for this line."));
    const { user } = await renderApp(projectPath(`models?model=${LINE}`));
    const sheet = await screen.findByRole("dialog", { name: /gpt-latest/ });
    expect(await within(sheet).findByText("KPMR moved gpt-latest to gpt-6")).toBeInTheDocument();
    expect(within(sheet).getByText("This project asks before the line moves.")).toBeInTheDocument();
    expect(within(sheet).getByText(/No provider key for `openai`/)).toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: /Move back/ })).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Keep" }));
    expect(await notifications().findByText("No move waits for this line.")).toBeInTheDocument();
    on("POST", `${LINES}/:model/keep`, line({ policy: "ask", kept: "gpt-6" }));
    await user.click(within(sheet).getByRole("button", { name: "Keep" }));
    expect(await notifications().findByText("gpt-latest stays on gpt-5.5")).toBeInTheDocument();
    await user.click(within(sheet).getByRole("button", { name: "Move" }));
    expect(await notifications().findByText("gpt-latest points at gpt-6 now")).toBeInTheDocument();
  });

  it("say when a delayed move happens and where KPMR points meanwhile, read-only without the right", async () => {
    setUp(
      [
        line({
          policy: "delay",
          delay_days: 3,
          kpmr_model: "gpt-6",
          pending: { model: "gpt-6", since: "2026-10-14T00:00:00Z", moves_at: "2026-10-17T00:00:00Z" },
        }),
      ],
      ["models.view"],
    );
    const { user } = await renderApp(projectPath(`models?model=${LINE}`));
    const sheet = await screen.findByRole("dialog", { name: /gpt-latest/ });
    expect(await within(sheet).findByText(/It moves on/)).toBeInTheDocument();
    expect(within(sheet).queryByRole("button", { name: "Move" })).not.toBeInTheDocument();
    expect(within(sheet).getByRole("spinbutton", { name: "Days to wait" })).toBeDisabled();
    await user.keyboard("{Escape}");
  });

  it("name where KPMR points when the project hasn't followed yet", async () => {
    setUp([line({ policy: "pin", kpmr_model: "gpt-6" })]);
    await renderApp(projectPath(`models?model=${LINE}`));
    const sheet = await screen.findByRole("dialog", { name: /gpt-latest/ });
    expect(await within(sheet).findByText("KPMR points at")).toBeInTheDocument();
    expect(within(sheet).getByText("gpt-6")).toBeInTheDocument();
  });
});
