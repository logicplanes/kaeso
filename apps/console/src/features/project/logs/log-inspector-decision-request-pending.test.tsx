import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { signedIn } from "~/test/fixtures";
import { projectData, projectPath } from "~/test/fixtures/project";
import { logContent, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { renderApp } from "~/test/render";

const questionsGate = vi.hoisted(() => {
  let open: () => void = () => {};
  const opened = new Promise<void>((resolve) => (open = resolve));
  return { opened, open: () => open() };
});

vi.mock("./conversation", async (importOriginal) => {
  await questionsGate.opened;
  return importOriginal();
});

describe("LogInspector while a decision's questions load", () => {
  it("holds the room of the text and each question, then shows them in the app's order", async () => {
    signedIn();
    projectData();
    const asked = {
      model: "jev-latest",
      state: "Charged twice.\n\nPlease refund me.",
      questions: { team: { type: "choice", criteria: { support: null, billing: "Payments." } }, refund: { type: "noul" } },
    };
    logsData({ rows: [logRow()], entries: [logEntry({ ingress_dialect: "system_one", content: logContent(asked, null) })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));
    const sheet = await screen.findByRole("dialog", { name: /^\d{3}/ });

    await user.click(within(sheet).getByRole("tab", { name: "Request" }));
    const tab = await within(sheet).findByRole("tabpanel");
    expect(tab.querySelectorAll("[aria-busy=true]")).toHaveLength(2);

    questionsGate.open();
    const asks = await within(tab).findByRole("list", { name: "Questions" });
    const [team, refund] = within(asks).getAllByRole("listitem");
    expect(team).toHaveAccessibleName("team");
    expect(refund).toHaveAccessibleName("refund");
    expect(within(tab).getByText("Please refund me.")).toBeInTheDocument();
    expect(tab.querySelector("[aria-busy=true]")).toBeNull();
  });
});
