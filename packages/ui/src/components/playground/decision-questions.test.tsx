import { describe, expect, it } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { LocaleProvider } from "../../i18n/provider.js";
import de from "../../i18n/locales/de.js";
import zh from "../../i18n/locales/zh-CN.js";
import { DecisionQuestions } from "./decision-questions.js";
import type { DecisionQuestion } from "./decision-types.js";

const questions: DecisionQuestion[] = [
  { name: "is_invoice", kind: "predicate", instructions: "Is this an invoice?", whenTrue: "It asks for a payment", whenFalse: "Anything else" },
  {
    name: "team",
    kind: "choice",
    instructions: "Which team handles it?",
    options: [{ value: "finance", description: "Invoices and refunds" }, { value: "legal" }],
  },
  { name: "urgency", kind: "score", levels: [{ label: "Low", description: "Can wait" }, { label: "High" }, {}] },
  { name: "total", kind: "box", instructions: "Where is the total?" },
  { kind: "predicate", instructions: "Is it signed?" },
];

function terms(item: HTMLElement): string[][] {
  return within(item)
    .queryAllByRole("term")
    .map((dt) => [dt.textContent ?? "", dt.nextElementSibling?.textContent ?? ""]);
}

describe("DecisionQuestions", () => {
  it("lists every question in order with its type, instructions and what its answers mean", () => {
    render(<DecisionQuestions questions={questions} />);
    const list = screen.getByRole("list", { name: "Questions" });
    const [invoice, team, urgency, total, fifth] = within(list).getAllByRole("listitem");
    expect(invoice).toHaveAccessibleName("is_invoice");
    expect(within(invoice!).getByText("Yes/no")).toBeInTheDocument();
    expect(within(invoice!).getByText("Is this an invoice?")).toBeInTheDocument();
    expect(terms(invoice!)).toEqual([
      ["Yes", "It asks for a payment"],
      ["No", "Anything else"],
    ]);
    expect(within(team!).getByText("2 options")).toBeInTheDocument();
    expect(terms(team!)).toEqual([
      ["finance", "Invoices and refunds"],
      ["legal", "No description"],
    ]);
    expect(within(team!).getByText("finance")).toHaveClass("font-mono");
    expect(within(urgency!).getByText("3 levels")).toBeInTheDocument();
    expect(within(urgency!).getByText("No instructions")).toBeInTheDocument();
    expect(terms(urgency!)).toEqual([
      ["0", "Low: Can wait"],
      ["1", "High"],
      ["2", "No description"],
    ]);
    expect(within(total!).getByText("Box")).toBeInTheDocument();
    expect(terms(total!)).toEqual([]);
    expect(fifth).toHaveAccessibleName("Question 5");
    expect(terms(fifth!)).toEqual([]);
  });

  it("shows a yes/no question's meanings when only one is given, and says the other is missing", () => {
    render(<DecisionQuestions questions={[{ name: "spam", kind: "predicate", whenTrue: "Advertising" }]} aria-label="Asked" />);
    expect(terms(screen.getByRole("listitem", { name: "spam" }))).toEqual([
      ["Yes", "Advertising"],
      ["No", "No description"],
    ]);
    expect(screen.getByRole("list", { name: "Asked" })).toBeInTheDocument();
  });

  it("speaks German and Chinese", () => {
    const { unmount } = render(
      <LocaleProvider locale="de" messages={de}>
        <DecisionQuestions questions={questions.slice(1, 3)} />
      </LocaleProvider>,
    );
    expect(screen.getByRole("list", { name: "Fragen" })).toBeInTheDocument();
    expect(screen.getByText("2 Optionen")).toBeInTheDocument();
    expect(screen.getByText("Keine Anweisungen")).toBeInTheDocument();
    expect(screen.getAllByText("Keine Beschreibung")).toHaveLength(2);
    unmount();
    render(
      <LocaleProvider locale="zh-CN" messages={zh}>
        <DecisionQuestions questions={questions.slice(1, 3)} />
      </LocaleProvider>,
    );
    expect(screen.getByRole("list", { name: "问题" })).toBeInTheDocument();
    expect(screen.getByText("3 个等级")).toBeInTheDocument();
    expect(screen.getByText("无说明")).toBeInTheDocument();
  });
});
