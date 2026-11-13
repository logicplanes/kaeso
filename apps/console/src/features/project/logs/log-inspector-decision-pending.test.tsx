import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { signedIn } from "~/test/fixtures";
import { projectData, projectPath } from "~/test/fixtures/project";
import { logContent, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { renderApp } from "~/test/render";

const answersGate = vi.hoisted(() => {
  let open: () => void = () => {};
  const opened = new Promise<void>((resolve) => (open = resolve));
  return { opened, open: () => open() };
});

vi.mock("./conversation", async (importOriginal) => {
  await answersGate.opened;
  return importOriginal();
});

describe("LogInspector while a decision's answers load", () => {
  it("holds a row for each question, then draws the answers it read without the request's questions", async () => {
    signedIn();
    projectData();
    const reply = {
      format: 2,
      decide: { model: "gpt-6-luna", answers: [{ value: { predicate: { probability: 0.82 } } }, { value: { refusal: {} } }], usage: {} },
    };
    logsData({
      rows: [logRow()],
      entries: [logEntry({ ingress_dialect: "openai_decisions", content: logContent({ model: "gpt-6-luna", input: "Hi", questions: "?" }, reply) })],
    });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));
    const sheet = await screen.findByRole("dialog", { name: /^\d{3}/ });

    await user.click(within(sheet).getByRole("tab", { name: "Reply" }));
    const tab = await within(sheet).findByRole("tabpanel");
    expect(tab.querySelector("[aria-busy=true]")).not.toBeNull();

    answersGate.open();
    const answers = await within(tab).findByRole("article", { name: "Answers" });
    expect(answers).toHaveTextContent("Question 1");
    expect(answers).toHaveTextContent("Question 2");
    expect(tab.querySelector("[aria-busy=true]")).toBeNull();
  });
});
