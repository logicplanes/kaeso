import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { LocaleProvider } from "../../i18n/provider.js";
import de from "../../i18n/locales/de.js";
import zh from "../../i18n/locales/zh-CN.js";
import { DecisionResponse } from "./decision-response.js";
import { ProbabilityBar } from "./probability-bar.js";
import { QuestionEditor } from "./question-editor.js";
import { ResponseMetrics } from "./response-metrics.js";
import type { DecisionAnswer, DecisionQuestion } from "./decision-types.js";

const PIXEL = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

const questions: DecisionQuestion[] = [
  { id: "a", name: "is_invoice", kind: "predicate", instructions: "Is this document an invoice?" },
  { id: "b", name: "department", kind: "choice", options: [{ value: "finance" }, { value: "legal" }, { value: "support" }] },
  { id: "c", name: "urgency", kind: "score", levels: [{ label: "Low" }, { label: "Medium" }, { label: "High" }] },
  { id: "d", name: "total", kind: "box" },
];
const answers: DecisionAnswer[] = [
  { kind: "predicate", probability: 0.9312 },
  { kind: "choice", option: 0, probabilities: [0.71, null, 0.002], confidence: 0.565 },
  { kind: "score", score: 1.62, probabilities: [0.08, 0.22, 0.7], confidence: 0.4 },
  { kind: "box", top: 0.62, bottom: 0.7, left: 0.55, right: 0.9 },
];

describe("ProbabilityBar", () => {
  it("is a meter named after its label with the value as text, and marks the chosen one in words", () => {
    const { rerender } = render(<ProbabilityBar label="finance" value={0.714} />);
    const meter = screen.getByRole("meter", { name: "finance" });
    expect(meter).toHaveAttribute("aria-valuenow", "71.4");
    expect(meter).toHaveAttribute("aria-valuetext", "71%");
    expect(meter).toHaveTextContent("71%");
    rerender(<ProbabilityBar label="finance" value={0.714} highlight layout="stacked" />);
    expect(screen.getByRole("meter", { name: "finance" })).toHaveAttribute("aria-valuetext", "71%, chosen");
    expect(screen.getByRole("meter")).toHaveAttribute("data-highlight", "true");
  });

  it("never rounds a small chance to 0% or a near-certain one to 100%", () => {
    const { rerender } = render(<ProbabilityBar label="legal" value={0.0004} />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "<1%");
    rerender(<ProbabilityBar label="legal" value={0.9987} />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", ">99%");
    rerender(<ProbabilityBar label="legal" value={1} />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "100%");
    rerender(<ProbabilityBar label="legal" value={0} />);
    expect(screen.getByRole("meter")).toHaveAttribute("aria-valuetext", "0%");
  });

  it("says a probability the provider left out wasn't given, and takes a name when the label isn't text", () => {
    const { container, rerender } = render(<ProbabilityBar label="support" value={null} />);
    expect(screen.queryByRole("meter")).toBeNull();
    expect(screen.getByText("Not given")).toBeInTheDocument();
    rerender(<ProbabilityBar label={<b>support</b>} value={0.5} aria-label="Support" />);
    expect(screen.getByRole("meter", { name: "Support" })).toBeInTheDocument();
    rerender(<ProbabilityBar label={<b>support</b>} value={0.5} />);
    expect(screen.getByRole("meter", { name: "support" })).toBeInTheDocument();
    rerender(<ProbabilityBar label="support" pending />);
    expect(screen.queryByRole("meter")).toBeNull();
    expect(container.querySelector(".animate-pulse-quiet")).not.toBeNull();
    expect(screen.queryByText("Not given")).toBeNull();
  });
});

describe("DecisionResponse", () => {
  it("holds a row per question while it waits, then shows every answer with its probabilities", () => {
    const { container, rerender } = render(
      <DecisionResponse questions={questions} status="pending" image={{ src: PIXEL, alt: "A receipt" }} aria-label="GPT-6 Luna response" />,
    );
    const article = screen.getByRole("article", { name: "GPT-6 Luna response" });
    expect(article).toHaveAttribute("aria-busy", "true");
    expect(article).toHaveAccessibleDescription("Waiting for the answers");
    expect(within(article).getAllByRole("listitem")).toHaveLength(4);
    expect(screen.queryAllByRole("meter")).toHaveLength(0);
    const waitingBars = container.querySelectorAll("[data-probability-bar]");
    expect(waitingBars).toHaveLength(2 + 3 + 3);
    expect(container.querySelector("[data-box]")).toBeNull();

    rerender(
      <DecisionResponse
        questions={questions}
        answers={answers}
        image={{ src: PIXEL, alt: "A receipt" }}
        footer={<ResponseMetrics duration={640} cost="0.0004" />}
        onRetry={() => undefined}
        aria-label="GPT-6 Luna response"
      />,
    );
    expect(container.querySelectorAll("[data-probability-bar]")).toHaveLength(waitingBars.length);
    expect(screen.getByRole("status")).toHaveTextContent("GPT-6 Luna response finished");
    const [invoice, department, urgency, total] = screen.getAllByRole("listitem");
    expect(invoice).toHaveAccessibleName("is_invoice");
    expect(within(invoice!).getByText("Yes/no")).toBeInTheDocument();
    expect(within(invoice!).getByRole("meter", { name: "Yes" })).toHaveAttribute("aria-valuetext", "93%, chosen");
    expect(within(invoice!).getByRole("meter", { name: "No" })).toHaveAttribute("aria-valuetext", "7%");

    expect(within(department!).getByText("Choice")).toBeInTheDocument();
    expect(within(department!).getByRole("meter", { name: "finance" })).toHaveAttribute("aria-valuetext", "71%, chosen");
    expect(within(department!).getByText("Not given")).toBeInTheDocument();
    expect(within(department!).getByRole("meter", { name: "support" })).toHaveAttribute("aria-valuetext", "<1%");
    expect(within(department!).getByText("Confidence 57%")).toBeInTheDocument();

    expect(within(urgency!).getByText("1.62 of 2, nearest High")).toBeInTheDocument();
    expect(within(urgency!).getByRole("meter", { name: "High" })).toHaveAttribute("aria-valuetext", "70%, chosen");

    expect(within(total!).getByRole("img", { name: "A receipt" })).toBeInTheDocument();
    const box = total!.querySelector<HTMLElement>("[data-box]");
    expect(parseFloat(box?.style.top ?? "")).toBeCloseTo(62);
    expect(parseFloat(box?.style.left ?? "")).toBeCloseTo(55);
    expect(parseFloat(box?.style.width ?? "")).toBeCloseTo(35);
    expect(parseFloat(box?.style.height ?? "")).toBeCloseTo(8);
    expect(within(total!).getByText("Top").nextElementSibling).toHaveTextContent("62%");
    expect(within(total!).getByText("Right").nextElementSibling).toHaveTextContent("90%");
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
  });

  it("shows a box's edges as numbers without an image, puts swapped edges in order, and reads a no", () => {
    const { container } = render(
      <DecisionResponse
        questions={[
          { kind: "box", name: "signature" },
          { kind: "predicate", name: "signed" },
        ]}
        answers={[
          { kind: "box", top: 0.4, bottom: 0.2, left: 0.1, right: 0.3, confidence: 0.92 },
          { kind: "predicate", probability: 0.12 },
        ]}
      />,
    );
    expect(screen.queryByRole("img")).toBeNull();
    expect(container.querySelector("[data-box]")).toBeNull();
    expect(screen.getByText("Top").nextElementSibling).toHaveTextContent("20%");
    expect(screen.getByText("Bottom").nextElementSibling).toHaveTextContent("40%");
    expect(within(screen.getAllByRole("listitem")[0]!).getByText("Confidence 92%")).toBeInTheDocument();
    expect(screen.getAllByText("No")).toHaveLength(1);
    expect(screen.getByRole("meter", { name: "No" })).toHaveAttribute("aria-valuetext", "88%, chosen");
    expect(screen.getByRole("meter", { name: "Yes" })).toHaveAttribute("aria-valuetext", "12%");
  });

  it("says when a question was refused, and when an answer is missing or of the wrong kind", () => {
    render(
      <DecisionResponse
        questions={[
          { kind: "predicate", name: "unsafe" },
          { kind: "choice", options: [{ value: "a" }, { value: "b" }] },
          { kind: "score", levels: [{}, {}] },
          { kind: "box" },
        ]}
        answers={[
          { kind: "refusal", reason: "The question asks for personal data." },
          { kind: "predicate", probability: 0.5 },
        ]}
      />,
    );
    const [unsafe, second, third, fourth] = screen.getAllByRole("listitem");
    expect(unsafe).toHaveAttribute("data-refused");
    expect(unsafe).toHaveTextContent("Refused: the model declined to answer this question");
    expect(unsafe).toHaveTextContent("The question asks for personal data.");
    expect(within(unsafe!).queryByRole("meter")).toBeNull();
    expect(second).toHaveAccessibleName("Question 2");
    expect(within(second!).getByText("No answer")).toBeInTheDocument();
    expect(within(second!).queryByText("Not given")).toBeNull();
    expect(within(second!).queryByText("a")).toBeNull();
    expect(within(third!).getByText("No answer")).toBeInTheDocument();
    expect(within(third!).queryByText("Level 0")).toBeNull();
    expect(within(fourth!).getByText("No answer")).toBeInTheDocument();
  });

  it("names each question with its type and confidence, shows instructions only when asked, and lines its bars up in one grid", () => {
    const questions = [
      {
        kind: "choice" as const,
        name: "team",
        instructions: "Which team should handle it?",
        options: [{ value: "finance" }, { value: "a much longer option" }],
      },
    ];
    const answers = [{ kind: "choice" as const, option: 0, probabilities: [0.8, 0.2], confidence: 0.6 }];
    const { container, rerender } = render(<DecisionResponse questions={questions} answers={answers} />);
    expect(screen.queryByText("Which team should handle it?")).toBeNull();
    const team = screen.getByRole("listitem", { name: "team" });
    expect(within(team).getByText("Confidence 60%")).toBeInTheDocument();
    expect(within(team).getByText("Choice")).toBeInTheDocument();
    expect(screen.getAllByText("finance")).toHaveLength(1);
    const grid = container.querySelector(".grid-cols-meter");
    expect(grid?.querySelectorAll("[data-probability-bar]")).toHaveLength(2);
    for (const bar of grid!.querySelectorAll("[data-probability-bar]")) expect(bar).toHaveClass("grid-cols-subgrid", "col-span-full");
    rerender(<DecisionResponse questions={questions} answers={answers} showInstructions />);
    expect(within(screen.getByRole("listitem", { name: "team" })).getByText("Which team should handle it?")).toBeInTheDocument();
  });

  it("shows the most likely options of a long choice first, and all of them on request", async () => {
    const options = Array.from({ length: 12 }, (_, i) => ({ value: `team-${i + 1}` }));
    const probabilities = options.map((_, i) => (i === 11 ? 0.3 : i === 0 ? 0.001 : 0.06));
    render(<DecisionResponse questions={[{ kind: "choice", name: "team", options }]} answers={[{ kind: "choice", option: 0, probabilities }]} maxBars={4} />);
    expect(screen.getAllByRole("meter").map((m) => m.getAttribute("aria-label"))).toEqual(["team-1", "team-2", "team-3", "team-12"]);
    const more = screen.getByRole("button", { name: "Show all 12 options" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(more);
    expect(screen.getAllByRole("meter")).toHaveLength(12);
    await userEvent.click(screen.getByRole("button", { name: "Show fewer" }));
    expect(screen.getAllByRole("meter")).toHaveLength(4);
  });

  it("collapses long scales too, and holds the same room while it waits", () => {
    const levels = Array.from({ length: 10 }, () => ({}));
    const { container, rerender } = render(<DecisionResponse questions={[{ kind: "score", levels }]} status="pending" maxBars={5} />);
    expect(container.querySelectorAll("[data-probability-bar]")).toHaveLength(5);
    expect(screen.getByRole("button", { name: "Show all 10 levels" })).toBeDisabled();
    rerender(<DecisionResponse questions={[{ kind: "score", levels }]} answers={[{ kind: "score", score: 4 }]} maxBars={5} />);
    expect(container.querySelectorAll("[data-probability-bar]")).toHaveLength(5);
    expect(screen.getByText("4 of 9")).toBeInTheDocument();
    expect(screen.getAllByText("Not given")).toHaveLength(5);
  });

  it("shows a failure in place of the answers, with Retry", async () => {
    const retry = vi.fn();
    render(<DecisionResponse questions={questions} status="error" error="The provider refused the request as invalid." onRetry={retry} />);
    expect(screen.getByRole("alert")).toHaveTextContent("No answers");
    expect(screen.getByRole("alert")).toHaveTextContent("The provider refused the request as invalid.");
    expect(screen.queryByRole("listitem")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(retry).toHaveBeenCalledOnce();
  });

  it("speaks German and Chinese", () => {
    const { rerender } = render(
      <LocaleProvider locale="de" messages={de}>
        <DecisionResponse questions={questions.slice(0, 3)} answers={answers} />
      </LocaleProvider>,
    );
    expect(screen.getByRole("meter", { name: "Ja" })).toHaveAttribute("aria-valuetext", "93\u00a0%, gewählt");
    expect(screen.getByText("Nicht angegeben")).toBeInTheDocument();
    expect(screen.getByText("Ja/Nein")).toBeInTheDocument();
    rerender(
      <LocaleProvider locale="zh-CN" messages={zh}>
        <DecisionResponse questions={questions.slice(0, 3)} answers={answers} />
      </LocaleProvider>,
    );
    expect(screen.getByRole("meter", { name: "是" })).toHaveAttribute("aria-valuetext", "93%，已选");
    expect(screen.getByText("置信度 57%")).toBeInTheDocument();
  });
});

function Editor({ start = [], allowBox = false, onChange }: { start?: DecisionQuestion[]; allowBox?: boolean; onChange?: (q: DecisionQuestion[]) => void }) {
  const [value, setValue] = useState<DecisionQuestion[]>(start);
  return (
    <>
      <QuestionEditor
        value={value}
        allowBox={allowBox}
        maxOptions={3}
        onValueChange={(next) => {
          setValue(next);
          onChange?.(next);
        }}
      />
      <output>{JSON.stringify(value.map(({ id: _id, ...q }) => q))}</output>
    </>
  );
}
const state = () => JSON.parse(screen.getByRole("status").textContent ?? "[]") as DecisionQuestion[];

describe("QuestionEditor", () => {
  it("adds a yes/no question with labelled fields, focusing its name", async () => {
    render(<Editor />);
    expect(screen.getByRole("group", { name: "Questions" })).toHaveTextContent("No questions yet.");
    await userEvent.click(screen.getByRole("button", { name: "Add question" }));
    const name = screen.getByRole("textbox", { name: "Name" });
    expect(name).toHaveFocus();
    await userEvent.keyboard("is_invoice");
    await userEvent.type(screen.getByRole("textbox", { name: "Instructions" }), "Is this an invoice?");
    await userEvent.type(screen.getByRole("textbox", { name: "What yes means" }), "It asks for payment");
    await userEvent.type(screen.getByRole("textbox", { name: "What no means" }), "Anything else");
    expect(screen.getByRole("group", { name: "is_invoice" })).toBeInTheDocument();
    expect(state()).toEqual([
      { kind: "predicate", name: "is_invoice", instructions: "Is this an invoice?", whenTrue: "It asks for payment", whenFalse: "Anything else" },
    ]);
    await userEvent.clear(screen.getByRole("textbox", { name: "What no means" }));
    expect(state()[0]).not.toHaveProperty("whenFalse");
  });

  it("edits a choice's options from the keyboard: Enter adds the next, the limit stops it, removing keeps two", async () => {
    render(<Editor start={[{ id: "q", kind: "predicate" }]} />);
    await userEvent.click(screen.getByRole("combobox", { name: "Type" }));
    expect(screen.queryByRole("option", { name: "Box" })).toBeNull();
    await userEvent.click(await screen.findByRole("option", { name: "Choice" }));
    expect(screen.getByRole("group", { name: "Options" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove option 1" })).toBeDisabled();
    await userEvent.click(screen.getByRole("textbox", { name: "Option 1" }));
    await userEvent.keyboard("finance");
    await userEvent.tab();
    expect(screen.getByRole("textbox", { name: "What option 1 means" })).toHaveFocus();
    await userEvent.keyboard("Invoices and payments");
    await userEvent.click(screen.getByRole("textbox", { name: "Option 2" }));
    await userEvent.keyboard("legal{Enter}");
    expect(screen.getByRole("textbox", { name: "Option 3" })).toHaveFocus();
    await userEvent.keyboard("support{Enter}");
    expect(screen.queryByRole("textbox", { name: "Option 4" })).toBeNull();
    expect(screen.getByRole("button", { name: "Add option" })).toBeDisabled();
    expect(screen.getByText("Up to 3 options")).toBeInTheDocument();
    expect(state()[0]?.options).toEqual([{ value: "finance", description: "Invoices and payments" }, { value: "legal" }, { value: "support" }]);
    await userEvent.click(screen.getByRole("button", { name: "Remove option 2" }));
    expect(screen.getByRole("textbox", { name: "Option 1" })).toHaveFocus();
    expect(state()[0]?.options).toEqual([{ value: "finance", description: "Invoices and payments" }, { value: "support" }]);
    await userEvent.click(screen.getByRole("button", { name: "Add option" }));
    expect(screen.getByRole("textbox", { name: "Option 3" })).toHaveFocus();
  });

  it("edits a score's levels from 0 up, and keeps them when the type changes back", async () => {
    render(<Editor start={[{ id: "q", kind: "score", name: "urgency", levels: [{}, {}, {}] }]} />);
    expect(screen.getByRole("group", { name: "Levels, from 0 up" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Label of level 0" })).toHaveAttribute("placeholder", "Level 0");
    await userEvent.type(screen.getByRole("textbox", { name: "Label of level 0" }), "Low");
    await userEvent.type(screen.getByRole("textbox", { name: "What level 2 means" }), "Needs an answer today");
    await userEvent.click(screen.getByRole("textbox", { name: "Label of level 2" }));
    await userEvent.keyboard("High{Enter}");
    expect(screen.getByRole("textbox", { name: "Label of level 3" })).toHaveFocus();
    await userEvent.click(screen.getByRole("button", { name: "Add level" }));
    expect(screen.getAllByRole("textbox", { name: /^Label of level/ })).toHaveLength(5);
    await userEvent.click(screen.getByRole("button", { name: "Remove level 4" }));
    await userEvent.click(screen.getByRole("button", { name: "Remove level 3" }));
    expect(state()[0]?.levels).toEqual([{ label: "Low" }, {}, { label: "High", description: "Needs an answer today" }]);
    await userEvent.click(screen.getByRole("combobox", { name: "Type" }));
    await userEvent.click(await screen.findByRole("option", { name: "Yes/no" }));
    expect(screen.queryByRole("group", { name: "Levels, from 0 up" })).toBeNull();
    await userEvent.click(screen.getByRole("combobox", { name: "Type" }));
    await userEvent.click(await screen.findByRole("option", { name: "Score" }));
    expect(screen.getByRole("textbox", { name: "Label of level 0" })).toHaveValue("Low");
  });

  it("offers Box only for a model that draws boxes, and removes questions with focus on a neighbour", async () => {
    render(
      <Editor
        allowBox
        start={[
          { id: "a", kind: "predicate", name: "signed" },
          { id: "b", kind: "box", name: "signature" },
        ]}
      />,
    );
    expect(screen.getByText(/answers with a box around/)).toBeInTheDocument();
    await userEvent.click(screen.getAllByRole("combobox", { name: "Type" })[0]!);
    expect(await screen.findByRole("option", { name: "Box" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    await userEvent.click(screen.getByRole("button", { name: "Remove signed" }));
    expect(screen.getByRole("button", { name: "Remove signature" })).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Add question" })).toHaveFocus();
    expect(state()).toEqual([]);
  });

  it("keeps Box in the list for a question that has it, stops at the question limit and turns off", async () => {
    const { rerender } = render(<QuestionEditor value={[{ kind: "box" }]} onValueChange={() => undefined} maxQuestions={1} />);
    expect(screen.getByRole("group", { name: "Question 1" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add question" })).toBeDisabled();
    expect(screen.getByText("Up to 1 question")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("combobox", { name: "Type" }));
    expect(await screen.findByRole("option", { name: "Box" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    rerender(<QuestionEditor value={[{ kind: "choice" }]} onValueChange={() => undefined} disabled aria-label="Decision questions" layout="compact" />);
    expect(screen.getByRole("group", { name: "Decision questions" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Name" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Add option" })).toBeDisabled();
  });

  it("speaks German", () => {
    render(
      <LocaleProvider locale="de" messages={de}>
        <QuestionEditor value={[{ kind: "score", levels: [{}, {}] }]} onValueChange={() => undefined} />
      </LocaleProvider>,
    );
    expect(screen.getByRole("group", { name: "Frage 1" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Bezeichnung von Stufe 0" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stufe hinzufügen" })).toBeInTheDocument();
  });
});
