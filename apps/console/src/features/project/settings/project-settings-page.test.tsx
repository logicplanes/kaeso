import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ORG, OTHER_PROJECT, PROJECT, org, project, signedIn, without } from "~/test/fixtures";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { logSettings, projectData, projectPath } from "~/test/fixtures/project";
import { apiError, on, requests } from "~/test/network";
import { renderApp } from "~/test/render";

const PROJECT_URL = "orgs/:org/projects/:project";
const LOG_SETTINGS = "orgs/:org/projects/:project/logs/settings";
const KEY_POLICY = "orgs/:org/projects/:project/key-policy";

function setUp({
  rights,
  view = project(),
  settings = logSettings(),
}: { rights?: string[]; view?: ReturnType<typeof project>; settings?: ReturnType<typeof logSettings> } = {}) {
  signedIn({ ...(rights ? { projectRights: rights } : {}), projects: [view, project({ id: OTHER_PROJECT, name: "Support triage", is_default: false })] });
  projectData();
  on("GET", LOG_SETTINGS, settings);
}

describe("project settings", () => {
  it("shows the project's name, id, creation date and that it is the default", async () => {
    setUp();
    await renderApp(projectPath("settings"));

    expect(await screen.findByRole("textbox", { name: "Project name" })).toHaveValue("Checkout assistant");
    expect(screen.getByText(PROJECT)).toBeInTheDocument();
    expect(screen.getByText("15 Oct 2026")).toBeInTheDocument();
    expect(screen.getByText("Default project")).toBeInTheDocument();
    expect(screen.getByText("New organization members join it; it can't be archived.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Upload" })).toBeInTheDocument();
  });

  it("renames the project with the trimmed name on Enter", async () => {
    setUp();
    on("PATCH", PROJECT_URL, ({ body }: { body: { name: string } }) => project({ name: body.name }));
    const { user } = await renderApp(projectPath("settings"));

    const input = await screen.findByRole("textbox", { name: "Project name" });
    await user.clear(input);
    await user.type(input, "  Checkout bot  {Enter}");
    expect(await notifications().findByText("Project renamed")).toBeInTheDocument();
    expect(requests("PATCH", PROJECT_URL).map((r) => r.body)).toEqual([{ name: "Checkout bot" }]);
  });

  it("reloads the project and the project list after a rename, not the project's other data", async () => {
    setUp();
    on("PATCH", PROJECT_URL, ({ body }: { body: { name: string } }) => project({ name: body.name }));
    const { user } = await renderApp(projectPath("settings"));

    const input = await screen.findByRole("textbox", { name: "Project name" });
    await screen.findByRole("switch", { name: "Log request content" });
    const before = {
      view: requests("GET", PROJECT_URL).length,
      list: requests("GET", "orgs/:org/projects").length,
      logging: requests("GET", LOG_SETTINGS).length,
    };
    await user.clear(input);
    await user.type(input, "Checkout bot{Enter}");
    expect(await notifications().findByText("Project renamed")).toBeInTheDocument();
    await waitFor(() => expect(requests("GET", PROJECT_URL).length).toBeGreaterThan(before.view));
    await waitFor(() => expect(requests("GET", "orgs/:org/projects").length).toBeGreaterThan(before.list));
    expect(requests("GET", LOG_SETTINGS)).toHaveLength(before.logging);
  });

  it("puts the name back when it is left empty or unchanged", async () => {
    setUp();
    const { user } = await renderApp(projectPath("settings"));

    const input = await screen.findByRole("textbox", { name: "Project name" });
    await user.click(input);
    await user.tab();
    await user.clear(input);
    await user.tab();
    expect(input).toHaveValue("Checkout assistant");
    expect(requests("PATCH", PROJECT_URL)).toHaveLength(0);
  });

  it("shows the API's message and keeps the typed name when the rename is refused", async () => {
    setUp();
    on("PATCH", PROJECT_URL, apiError(422, "invalid_request", "That name is taken.", { param: "name" }));
    const { user } = await renderApp(projectPath("settings"));

    const input = await screen.findByRole("textbox", { name: "Project name" });
    await user.clear(input);
    await user.type(input, "Support triage");
    await user.tab();
    expect((await notifications().findAllByText("That name is taken.")).length).toBeGreaterThan(0);
    expect(input).toHaveValue("Support triage");
  });

  it("sends one rename when Enter is followed by leaving the field", async () => {
    setUp();
    let answer: (view: unknown) => void = () => {};
    on("PATCH", PROJECT_URL, () => new Promise((resolve) => (answer = resolve)));
    const { user } = await renderApp(projectPath("settings"));

    const input = await screen.findByRole("textbox", { name: "Project name" });
    await user.clear(input);
    await user.type(input, "Checkout bot{Enter}");
    await user.tab();
    answer(project({ name: "Checkout bot" }));
    expect(await notifications().findByText("Project renamed")).toBeInTheDocument();
    expect(requests("PATCH", PROJECT_URL)).toHaveLength(1);
  });

  it("locks the name and picture without Edit project and hides the sections that need other rights", async () => {
    setUp({ rights: without("project.update", "security.key_policy.project.manage", "project.archive", "logs.content.manage") });
    const { user } = await renderApp(projectPath("settings"));

    const input = await screen.findByRole("textbox", { name: "Project name" });
    expect(input).toBeDisabled();
    const upload = screen.getByRole("button", { name: "Upload" });
    expect(upload).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, upload, /You need Edit project/);
    expect(screen.queryByText("Key lifetime")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Archive project" })).not.toBeInTheDocument();
    const logging = await screen.findByRole("switch", { name: "Log request content" });
    expect(logging).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, logging, /^You need Switch content logging; ask .+ for it\.$/);
  });

  it("keeps the name and picture of an archived project", async () => {
    setUp({ view: project({ archived_at: "2026-10-15T12:00:00Z", avatar_url: "/avatars/project.webp" }) });
    const { user } = await renderApp(projectPath("settings"));

    expect(await screen.findByRole("textbox", { name: "Project name" })).toBeDisabled();

    const replace = await screen.findByRole("button", { name: "Replace" });
    expect(replace).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, replace, "An archived project can't change");
    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
  });

  it("says when an archived project was archived instead of offering to archive it again", async () => {
    setUp({ view: project({ is_default: false, archived_at: "2026-10-19T12:00:00Z" }) });
    await renderApp(projectPath("settings"));

    expect(await screen.findByText("Its keys and service accounts went with it; history is kept.")).toBeInTheDocument();
    expect(screen.getByText("19 Oct 2026")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Archive project" })).not.toBeInTheDocument();
    expect(await screen.findByRole("switch", { name: "Use the organization's limit" })).toHaveAttribute("aria-disabled", "true");
    expect(await screen.findByRole("switch", { name: "Log request content" })).toHaveAttribute("aria-disabled", "true");
  });

  it("keeps an archived project's own key lifetime as it is", async () => {
    setUp({ view: project({ is_default: false, archived_at: "2026-10-19T12:00:00Z", max_key_lifetime_days: 30 }) });
    const { user } = await renderApp(projectPath("settings"));

    expect(await screen.findByRole("textbox", { name: "Maximum key lifetime" })).toBeDisabled();
    await hoverReason(user, screen.getByRole("button", { name: "Save" }), "An archived project can't change");
  });

  it("removes the picture", async () => {
    setUp({ view: project({ avatar_url: "/avatars/project.webp" }) });
    on("DELETE", "orgs/:org/projects/:project/avatar", project());
    const { user } = await renderApp(projectPath("settings"));

    await user.click(await screen.findByRole("button", { name: "Remove" }));
    expect(await notifications().findByText("Picture removed")).toBeInTheDocument();
    expect(requests("DELETE", "orgs/:org/projects/:project/avatar")).toHaveLength(1);
  });

  it("uploads a new picture", async () => {
    setUp();
    on("PUT", "orgs/:org/projects/:project/avatar", project({ avatar_url: "/avatars/project.webp" }));
    vi.stubGlobal(
      "createImageBitmap",
      vi.fn(async () => ({ width: 300, height: 300, close: vi.fn() })),
    );
    const context = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage: vi.fn(), imageSmoothingQuality: "low" } as never);
    const blob = vi
      .spyOn(HTMLCanvasElement.prototype, "toBlob")
      .mockImplementation((done: BlobCallback) => done(new Blob([new Uint8Array([7])], { type: "image/webp" })));
    try {
      const { user, container } = await renderApp(projectPath("settings"));
      await screen.findByRole("button", { name: "Upload" });
      const picker = container.ownerDocument.querySelector<HTMLInputElement>('input[type="file"]') as HTMLInputElement;
      await user.upload(picker, new File(["png"], "logo.png", { type: "image/png" }));
      expect(await notifications().findByText("Picture saved")).toBeInTheDocument();
      expect(requests("PUT", "orgs/:org/projects/:project/avatar").map((r) => r.body)).toEqual([{ content_type: "image/webp", data: btoa("\u0007") }]);
    } finally {
      context.mockRestore();
      blob.mockRestore();
      vi.unstubAllGlobals();
    }
  });
});

describe("key lifetime", () => {
  it("follows the organization's limit and sets one of the project's own", async () => {
    setUp();
    on("GET", "orgs/:org", org({ max_key_lifetime_days: 30 }));
    on("PUT", KEY_POLICY, ({ body }: { body: { max_key_lifetime_days: number | null } }) => body);
    const { user } = await renderApp(projectPath("settings"));

    expect(await screen.findByText("At most 30 days.")).toBeInTheDocument();
    const inherit = screen.getByRole("switch", { name: "Use the organization's limit" });
    expect(inherit).toBeChecked();
    expect(screen.queryByRole("textbox", { name: "Maximum key lifetime" })).not.toBeInTheDocument();
    await user.click(inherit);
    expect(await notifications().findByText("Key lifetime saved")).toBeInTheDocument();
    expect(requests("PUT", KEY_POLICY).map((r) => r.body)).toEqual([{ max_key_lifetime_days: 30 }]);
  });

  it("saves a lifetime within the organization's limit", async () => {
    setUp({ view: project({ max_key_lifetime_days: 14 }) });
    on("GET", "orgs/:org", org({ max_key_lifetime_days: 30 }));
    on("PUT", KEY_POLICY, ({ body }: { body: { max_key_lifetime_days: number | null } }) => body);
    const { user } = await renderApp(projectPath("settings"));

    const days = await screen.findByRole("textbox", { name: "Maximum key lifetime" });
    expect(days).toHaveValue("14");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await user.clear(days);
    await user.type(days, "45");
    await hoverReason(user, screen.getByRole("button", { name: "Save" }), "The organization allows at most 30 days.");
    await user.clear(days);
    await user.type(days, "0");
    await hoverReason(user, screen.getByRole("button", { name: "Save" }), "Enter whole days, at least 1.");
    await user.clear(days);
    await user.type(days, "21");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PUT", KEY_POLICY).map((r) => r.body)).toEqual([{ max_key_lifetime_days: 21 }]));

    await user.click(screen.getByRole("switch", { name: "Use the organization's limit" }));
    await waitFor(() => expect(requests("PUT", KEY_POLICY).map((r) => r.body)).toContainEqual({ max_key_lifetime_days: null }));
  });

  it("starts at 90 days when the organization sets no limit", async () => {
    setUp();
    on("PUT", KEY_POLICY, ({ body }: { body: { max_key_lifetime_days: number | null } }) => body);
    const { user } = await renderApp(projectPath("settings"));

    expect(await screen.findByText("The organization sets no limit.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Use the organization's limit" }));
    await waitFor(() => expect(requests("PUT", KEY_POLICY).map((r) => r.body)).toEqual([{ max_key_lifetime_days: 90 }]));
  });
});

describe("content logging", () => {
  it("turns content logging off and on", async () => {
    setUp();
    on("PATCH", LOG_SETTINGS, ({ body }: { body: { content_logging: boolean } }) =>
      logSettings({ content_logging: body.content_logging, effective_content_logging: body.content_logging }),
    );
    const { user } = await renderApp(projectPath("settings"));

    const toggle = await screen.findByRole("switch", { name: "Log request content" });
    await waitFor(() => expect(toggle).toBeChecked());
    expect(screen.getByText("Bodies are kept for the organization's content retention.")).toBeInTheDocument();
    await user.click(toggle);
    expect(await notifications().findByText("Content logging turned off")).toBeInTheDocument();
    expect(requests("PATCH", LOG_SETTINGS).map((r) => r.body)).toEqual([{ content_logging: false }]);
  });

  it("turns content logging on", async () => {
    setUp({ settings: logSettings({ content_logging: false, effective_content_logging: false }) });
    on("PATCH", LOG_SETTINGS, logSettings());
    const { user } = await renderApp(projectPath("settings"));

    expect(await screen.findByText("Only metadata is kept: timing, tokens, cost and errors.")).toBeInTheDocument();
    await user.click(screen.getByRole("switch", { name: "Log request content" }));
    expect(await notifications().findByText("Content logging turned on")).toBeInTheDocument();
  });

  it("can't be turned on when the organization turned it off", async () => {
    setUp({ settings: logSettings({ content_logging: true, effective_content_logging: false }) });
    await renderApp(projectPath("settings"));

    expect(await screen.findByText("Off for the organization; it can't be turned on here.")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Log request content" })).toHaveAttribute("aria-disabled", "true");
  });
});

describe("archiving the project", () => {
  it("can't archive the default project", async () => {
    setUp();
    const { user } = await renderApp(projectPath("settings"));

    const archive = await screen.findByRole("button", { name: "Archive project" });
    expect(archive).toHaveAttribute("aria-disabled", "true");
    await hoverReason(user, archive, "Make another project the default to archive this one");
  });

  it("archives the project once its name is typed", async () => {
    setUp({ view: project({ is_default: false }) });
    on("POST", "orgs/:org/projects/:project/archive", project({ is_default: false, archived_at: "2026-10-15T12:00:00Z" }));
    const { user, router } = await renderApp(projectPath("settings"));

    expect(screen.queryByText("Default project")).not.toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "Archive project" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Archive Checkout assistant?" });
    await user.type(within(dialog).getByRole("textbox"), "Checkout assistant");
    await user.click(within(dialog).getByRole("button", { name: "Archive project" }));
    expect(await notifications().findByText("Checkout assistant archived")).toBeInTheDocument();
    expect(requests("POST", "orgs/:org/projects/:project/archive")).toHaveLength(1);
    await waitFor(() => expect(router.state.location.pathname).not.toBe(projectPath("settings")));
    expect(router.state.location.pathname.startsWith(`/orgs/${ORG}`)).toBe(true);
  });
});
