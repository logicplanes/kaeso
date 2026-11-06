import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { PROJECT_TOOLS, TOOL_ACCESS, projectTool } from "~/test/fixtures/mcp";
import { on, page, requests } from "~/test/network";
import { renderUi } from "~/test/render";
import { ToolAccessSection } from "./tool-access-section";

const TOOLS = [projectTool(), projectTool({ name: "create_post" }), projectTool({ name: "search", offered: false })];

function access(byPrincipal: Record<string, { all: boolean; tools: string[]; effective: string[] }>) {
  on("GET", PROJECT_TOOLS, page(TOOLS));
  on("GET", TOOL_ACCESS, ({ params }: { params: Record<string, string> }) => byPrincipal[params.principal ?? ""]);
  on("PUT", TOOL_ACCESS, ({ body }: { body: { all?: boolean; tools?: string[] } }) => ({
    all: !!body.all,
    tools: body.tools ?? [],
    effective: body.tools ?? [],
  }));
}

describe("ToolAccessSection", () => {
  it("says a member may call every tool the project offers, and narrows it to chosen tools", async () => {
    signedIn();
    access({ usr_grace: { all: true, tools: [], effective: ["tool_get_me", "tool_create_post"] } });
    const { user } = await renderUi(<ToolAccessSection org={ORG} project={PROJECT} subject={{ principal: "usr_grace" }} canEdit />);
    expect(await screen.findByText("Every tool the project offers: 2 tools.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("switch", { name: "Every tool the project offers" }));
    const group = screen.getByRole("group", { name: "Tools" });
    expect(within(group).queryByRole("checkbox", { name: "x__search" })).not.toBeInTheDocument();
    await user.click(within(group).getByRole("checkbox", { name: "x__get_me" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await notifications().findByText("Tools saved")).toBeInTheDocument();
    expect(requests("PUT", TOOL_ACCESS)[0]?.body).toEqual({ tools: ["tool_get_me"] });
  });

  it("lists a key's chosen tools, offers its owner's tools and lets every tool through again", async () => {
    signedIn();
    access({
      key_web: { all: false, tools: ["tool_create_post", "tool_gone"], effective: ["tool_create_post"] },
      usr_ada: { all: true, tools: [], effective: ["tool_get_me", "tool_create_post"] },
    });
    const { user } = await renderUi(<ToolAccessSection org={ORG} project={PROJECT} subject={{ principal: "key_web", owner: "usr_ada" }} canEdit />);
    expect(await screen.findByText("x__create_post, tool_gone")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Edit" }));
    const group = await screen.findByRole("group", { name: "Tools" });
    await waitFor(() => expect(within(group).getByRole("checkbox", { name: "x__get_me" })).toBeInTheDocument());
    expect(within(group).getByRole("checkbox", { name: "tool_gone" })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.click(screen.getByRole("switch", { name: "Every tool its owner may use" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(requests("PUT", TOOL_ACCESS)[0]?.body).toEqual({ all: true }));
  });

  it("counts an owner's tools, says when there's nothing to call, and when nothing is chosen", async () => {
    signedIn();
    access({
      key_all: { all: true, tools: [], effective: ["tool_get_me"] },
      key_none: { all: true, tools: [], effective: [] },
      key_empty: { all: false, tools: [], effective: [] },
    });
    await renderUi(
      <>
        <ToolAccessSection org={ORG} project={PROJECT} subject={{ principal: "key_all", owner: "usr_ada" }} canEdit={false} />
        <ToolAccessSection org={ORG} project={PROJECT} subject={{ principal: "key_none" }} canEdit={false} />
        <ToolAccessSection org={ORG} project={PROJECT} subject={{ principal: "key_empty" }} canEdit={false} />
      </>,
    );
    expect(await screen.findByText("Every tool its owner may use: 1 tool.")).toBeInTheDocument();
    expect(await screen.findByText("No tools to call yet.")).toBeInTheDocument();
    expect(await screen.findByText("None")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit" })).not.toBeInTheDocument();
  });
});
