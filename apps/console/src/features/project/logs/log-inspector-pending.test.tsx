import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { signedIn } from "~/test/fixtures";
import { projectData, projectPath } from "~/test/fixtures/project";
import { logContent, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { renderApp } from "~/test/render";

const conversationGate = vi.hoisted(() => {
  let open: () => void = () => {};
  const opened = new Promise<void>((resolve) => (open = resolve));
  return { opened, open: () => open() };
});

vi.mock("./conversation", async (importOriginal) => {
  await conversationGate.opened;
  return importOriginal();
});

describe("LogInspector while the conversation view loads", () => {
  it("holds the place of the request and the reply, then draws them", async () => {
    signedIn();
    projectData();
    logsData({
      rows: [logRow()],
      entries: [
        logEntry({
          content: logContent(
            { model: "gpt-5.4-mini", messages: [{ role: "user", content: "Where is order 7?" }] },
            { blocks: [{ text: { text: "Order 7 shipped yesterday." } }], stop: "end_turn" },
          ),
        }),
      ],
    });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));
    const sheet = await screen.findByRole("dialog", { name: /^\d{3}/ });

    await user.click(within(sheet).getByRole("tab", { name: "Request" }));
    const request = await within(sheet).findByRole("tabpanel");
    expect(request.querySelector("[aria-busy=true]")).not.toBeNull();
    expect(within(request).queryByRole("region", { name: "User" })).not.toBeInTheDocument();

    await user.click(within(sheet).getByRole("tab", { name: "Reply" }));
    const reply = await within(sheet).findByRole("tabpanel");
    expect(reply.querySelector("[aria-busy=true]")).not.toBeNull();
    expect(within(reply).getByText("End of turn")).toBeInTheDocument();

    conversationGate.open();
    expect(await within(reply).findByRole("region", { name: "Assistant" })).toHaveTextContent("Order 7 shipped yesterday.");
    expect(reply.querySelector("[aria-busy=true]")).toBeNull();
    await user.click(within(sheet).getByRole("tab", { name: "Request" }));
    expect(await within(sheet).findByRole("region", { name: "User" })).toHaveTextContent("Where is order 7?");
  });
});
