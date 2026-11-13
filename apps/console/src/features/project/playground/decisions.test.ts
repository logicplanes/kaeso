import { describe, expect, it } from "vitest";
import type { DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import { playgroundModel } from "~/test/fixtures/project-logs";
import { answerOf, answersOf, decisionRequest, editorLimits, problemOf, questionKeys, stateOf, systemOneQuestion } from "./decisions";

const options = (...values: string[]) => values.map((value) => ({ value }));
const decides = (types: string[], limits: { max_questions?: number | null; max_options?: number | null; max_levels?: number | null } = {}) => ({
  question_types: types,
  max_questions: null,
  max_options: null,
  max_levels: null,
  max_images: null,
  images: true,
  max_request_bytes: 67_108_864,
  needs_api_key: false,
  ...limits,
});

describe("questionKeys", () => {
  it("keys each question by its name, and numbers the rest without taking a name", () => {
    const questions: DecisionQuestion[] = [{ kind: "predicate", name: "q2" }, { kind: "predicate" }, { kind: "box", name: " door " }, { kind: "score" }];
    expect(questionKeys(questions)).toEqual(["q2", "q1", "door", "q3"]);
  });
});

describe("problemOf", () => {
  it("finds what a decision model would refuse before anything is sent", () => {
    expect(
      problemOf([
        { kind: "predicate", name: "refund" },
        { kind: "predicate", name: "refund" },
      ]),
    ).toBe("Two questions are named refund; rename one.");
    expect(problemOf([{ kind: "choice", options: options("billing", " ") }])).toBe("Give every option of Question 1 a value.");
    expect(problemOf([{ kind: "choice", name: "team", options: options("billing", "billing ") }])).toBe("Two options of team have the same value.");
    expect(problemOf([{ kind: "score", levels: [{ label: "low" }, {}] }])).toBe("Describe every level of Question 1.");
    expect(problemOf([{ kind: "score", levels: [{ label: "low" }, { description: "Today" }] }, { kind: "predicate" }])).toBeNull();
  });
});

describe("stateOf", () => {
  const image = { id: "f1", name: "door.png", type: "image/png", data: "data:image/png;base64,AAAA" };
  it("sends text as text, a JSON object or array as JSON, and images as references after the text", () => {
    expect(stateOf("A parcel arrived broken.", [])).toBe("A parcel arrived broken.");
    expect(stateOf(' {"order": 4471} ', [])).toEqual({ order: 4471 });
    expect(stateOf("[1, 2]", [])).toEqual([1, 2]);
    expect(stateOf("{not json", [])).toBe("{not json");
    expect(stateOf("Is the door open?", [image, image])).toEqual(["Is the door open?", "<image-1>", "<image-2>"]);
    expect(stateOf(" ", [image])).toEqual(["<image-1>"]);
  });
});

describe("systemOneQuestion", () => {
  it("writes only the criteria a question has", () => {
    expect(systemOneQuestion({ kind: "predicate", whenFalse: "Anything else" })).toEqual({ type: "noul", criteria: { false: "Anything else" } });
    expect(systemOneQuestion({ kind: "predicate", whenTrue: " " })).toEqual({ type: "noul" });
    expect(systemOneQuestion({ kind: "choice" })).toEqual({ type: "choice", criteria: {} });
    expect(systemOneQuestion({ kind: "score" })).toEqual({ type: "score", criteria: [] });
    expect(problemOf([{ kind: "choice" }, { kind: "score" }])).toBeNull();
  });
});

describe("decisionRequest", () => {
  it("writes the questions as System One's and the images as its assets", () => {
    const jev = playgroundModel({ model: "jev-1.14.0", mode: "decision" });
    const body = decisionRequest(
      jev,
      [
        { kind: "predicate", name: "refund", instructions: "Do they want money back?", whenTrue: "They ask for a refund" },
        { kind: "choice", instructions: "Which team?", options: [{ value: "billing", description: "Payments" }, { value: "shipping" }] },
        { kind: "score", name: "urgency", levels: [{ label: "low" }, { label: "high", description: "Today" }, { description: "Now" }] },
        { kind: "box", name: "door", instructions: " ", whenTrue: "ignored" },
      ],
      "Is the door open?",
      [
        { id: "a", name: "a.jpg", type: "image/jpeg", data: "data:image/jpeg;base64,/9j/" },
        { id: "b", name: "b.png", type: "image/png", data: "data:image/png;base64,iVBO" },
      ],
    );
    expect(body).toEqual({
      model: "jev-1.14.0",
      state: ["Is the door open?", "<image-1>", "<image-2>"],
      questions: {
        refund: { type: "noul", instructions: "Do they want money back?", criteria: { true: "They ask for a refund" } },
        q1: { type: "choice", instructions: "Which team?", criteria: { billing: "Payments", shipping: null } },
        urgency: { type: "score", criteria: ["low", "high: Today", "Now"] },
        door: { type: "box" },
      },
      assets: {
        "<image-1>": { type: "image", format: "jpeg", data_base64: "/9j/" },
        "<image-2>": { type: "image", format: "png", data_base64: "iVBO" },
      },
    });
  });
});

describe("answersOf", () => {
  const questions: DecisionQuestion[] = [
    { kind: "predicate", name: "refund" },
    { kind: "choice", name: "team", options: options("billing", "shipping") },
    { kind: "score", name: "urgency", levels: [{ label: "low" }, { label: "high" }] },
    { kind: "box", name: "door" },
    { kind: "predicate", name: "polite" },
    { kind: "predicate", name: "missing" },
    { kind: "choice", name: "wrong", options: options("a", "b") },
  ];
  it("reads each answer under its question's key, in the order asked", () => {
    const reply = {
      answers: {
        refund: { type: "noul", noul: 0.82 },
        team: { type: "choice", choice: "shipping", confidence: 0.4, probabilities: { billing: 0.3, shipping: 0.7 } },
        urgency: { type: "score", score: 0.6, confidence: 0.2, probabilities: { "0": 0.4, "1": 0.6 } },
        door: { type: "box", box: { top: 0.1, bottom: 0.9, left: 0.2, right: 0.8 }, confidence: null },
        polite: { type: "refusal" },
        wrong: { type: "choice", choice: "c", probabilities: {} },
      },
    };
    expect(answersOf(reply.answers, questions)).toEqual([
      { kind: "predicate", probability: 0.82 },
      { kind: "choice", option: 1, probabilities: [0.3, 0.7], confidence: 0.4 },
      { kind: "score", score: 0.6, probabilities: [0.4, 0.6], confidence: 0.2 },
      { kind: "box", top: 0.1, bottom: 0.9, left: 0.2, right: 0.8 },
      { kind: "refusal" },
      null,
      null,
    ]);
  });
  it("leaves out answers of another type or without their values", () => {
    const reply = { answers: { refund: { type: "choice", choice: "x" }, urgency: { type: "score" }, door: { type: "box", box: { top: 0.1 } } } };
    expect(answersOf(reply.answers, questions).slice(0, 4)).toEqual([null, null, null, null]);
  });
  it("keeps a refusal's reason, and reads questions sent without options or levels", () => {
    expect(answerOf({ kind: "box" }, { type: "refusal", reason: "Unsafe image" })).toEqual({ kind: "refusal", reason: "Unsafe image" });
    expect(answerOf({ kind: "choice" }, { type: "choice", choice: "a" })).toBeNull();
    expect(answerOf({ kind: "score" }, { type: "score", score: 0 })).toEqual({ kind: "score", score: 0, probabilities: [] });
    expect(answerOf({ kind: "predicate" }, "yes")).toBeNull();
  });
});

describe("editorLimits", () => {
  it("takes the strictest limits of the models that answer, and box only when each draws boxes", () => {
    const jev = playgroundModel({ mode: "decision", decisions: decides(["predicate", "choice", "score", "box"], { max_options: 255, max_levels: 10 }) });
    const luna = playgroundModel({ decisions: decides(["predicate", "choice", "score"], { max_questions: 400, max_levels: 10 }) });
    const chat = playgroundModel();
    expect(editorLimits([jev, chat])).toEqual({ allowBox: true, maxQuestions: undefined, maxOptions: 255, maxLevels: 10 });
    expect(editorLimits([jev, luna])).toEqual({ allowBox: false, maxQuestions: 400, maxOptions: 255, maxLevels: 10 });
    expect(editorLimits([chat]).allowBox).toBe(false);
  });
});
