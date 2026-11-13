import { describe, expect, it } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import { PLAYGROUND_CONVERSATION } from "~/auth/user-data";
import { remember } from "~/shell/session-memory";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { apiError, json, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { eventStream, playgroundModel, sse } from "~/test/fixtures/project-logs";

const MODELS = "orgs/:org/projects/:project/playground/models";
const RESPONSES = "orgs/:org/projects/:project/playground/responses";
const DECISIONS = "orgs/:org/projects/:project/playground/decisions";
const PLAYGROUND = `/orgs/${ORG}/projects/${PROJECT}/playground`;
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

const decides = (types: string[], extra: Record<string, unknown> = {}) => ({
  question_types: types,
  max_questions: null,
  max_options: 255,
  max_levels: 10,
  max_images: null,
  images: true,
  max_request_bytes: 67_108_864,
  needs_api_key: false,
  ...extra,
});
const jev = playgroundModel({
  id: "cm_jev",
  provider: "typesafe",
  model: "jev-1.14.0",
  model_id: "jev-1.14.0",
  display_name: "Jev 1.14",
  mode: "decision",
  input_modalities: ["text", "image"],
  input_price: "0.042",
  output_price: "0",
  max_request_bytes: 67_108_864,
  decisions: decides(["predicate", "choice", "score", "box"]),
});
const luna = playgroundModel({
  id: "cm_luna",
  model: "gpt-6-luna",
  model_id: "gpt-6-luna",
  display_name: "GPT-6 Luna",
  decisions: decides(["predicate", "choice", "score"], { max_questions: 400 }),
});
const chat = playgroundModel();

const QUESTIONS: DecisionQuestion[] = [
  { id: "a", kind: "predicate", name: "refund", instructions: "Does the customer ask for a refund?" },
  { id: "b", kind: "choice", name: "team", instructions: "Which team handles it?", options: [{ value: "billing" }, { value: "shipping" }] },
];
const seed = (questions: DecisionQuestion[]) => remember([...PLAYGROUND_CONVERSATION, PROJECT, "questions"], questions);
const answers = {
  refund: { type: "noul", noul: 0.82 },
  team: { type: "choice", choice: "shipping", confidence: 0.4, probabilities: { billing: 0.3, shipping: 0.7 } },
};
const decided = (extra: Record<string, unknown> = {}) =>
  json({ model: "jev-1.14.0", answers, usage: { input_tokens: 120, output_tokens: 0 }, ...extra }, { headers: { "kaeso-cost": "0.000005" } });
const reply = (text: string) =>
  eventStream(
    sse([
      { type: "response.created" },
      { type: "response.output_text.delta", item_id: "m", delta: text },
      { type: "response.completed", response: { usage: { input_tokens: 3, output_tokens: 2 } } },
    ]),
  );
const open = (models: string) => renderApp(`${PLAYGROUND}?models=${models}`);
const send = async (user: Awaited<ReturnType<typeof renderApp>>["user"], text: string) => {
  await user.type(screen.getByRole("textbox", { name: "Prompt" }), text);
  await user.click(screen.getByRole("button", { name: "Send" }));
};

describe("Playground decisions", () => {
  it("asks a decision model the questions and sends a chat model the same text as its prompt", async () => {
    signedIn();
    seed(QUESTIONS);
    on("GET", MODELS, page([jev, chat]));
    on("POST", DECISIONS, () => decided());
    on("POST", RESPONSES, () => reply("Sounds like a refund."));
    const { user } = await open("typesafe/jev-1.14.0,openai/gpt-5.4-mini");

    const prompt = await screen.findByRole("textbox", { name: "Prompt" });
    await waitFor(() => expect(prompt).toHaveAttribute("placeholder", "Send what the questions are about to every model"));
    expect(screen.getByRole("button", { name: "Questions (2)" })).toBeInTheDocument();
    await send(user, "My parcel arrived broken; I want my money back.");

    const answer = await screen.findByRole("article", { name: "Jev 1.14 response" });
    await waitFor(() => expect(within(answer).getByText("Confidence 40%")).toBeInTheDocument());
    const refund = within(answer).getByRole("listitem", { name: "refund" });
    expect(within(refund).getAllByText("Yes")[0]).toBeInTheDocument();
    expect(within(refund).getByText("82%")).toBeInTheDocument();
    const team = within(answer).getByRole("listitem", { name: "team" });
    expect(within(team).getAllByText("shipping").length).toBeGreaterThan(0);
    expect(within(answer).getByText("$0.000005")).toBeInTheDocument();
    expect(requests("POST", DECISIONS)[0]?.body).toEqual({
      model: "jev-1.14.0",
      state: "My parcel arrived broken; I want my money back.",
      questions: {
        refund: { type: "noul", instructions: "Does the customer ask for a refund?" },
        team: { type: "choice", instructions: "Which team handles it?", criteria: { billing: null, shipping: null } },
      },
    });
    expect(await screen.findByText("Sounds like a refund.")).toBeInTheDocument();
    expect(JSON.stringify(requests("POST", RESPONSES)[0]?.body)).toContain("My parcel arrived broken");
  });

  it("builds the questions in the sheet, with box only when every model draws boxes", async () => {
    signedIn();
    on("GET", MODELS, page([jev, luna]));
    on("POST", DECISIONS, () => json({ model: "jev-1.14.0", answers: { q1: { type: "noul", noul: 0.1 } }, usage: { input_tokens: 9, output_tokens: 0 } }));
    const { user } = await open("typesafe/jev-1.14.0");

    await user.click(await screen.findByRole("button", { name: "Questions" }));
    const sheet = await screen.findByRole("dialog", { name: "Questions" });
    await user.click(within(sheet).getByRole("button", { name: "Add question" }));
    await user.click(within(sheet).getByRole("combobox", { name: "Type" }));
    expect(await screen.findByRole("option", { name: "Box" })).toBeInTheDocument();
    await user.click(screen.getByRole("option", { name: "Yes/no" }));
    await user.type(within(sheet).getByRole("textbox", { name: "Instructions" }), "Is it urgent?");
    expect(screen.getByRole("button", { name: "Questions (1)" })).toBeInTheDocument();
    await send(user, "Ship it today please");
    const answer = await screen.findByRole("article", { name: "Jev 1.14 response" });
    await waitFor(() => expect(within(answer).getAllByText("No").length).toBeGreaterThan(1));
    expect(requests("POST", DECISIONS)[0]?.body).toMatchObject({ questions: { q1: { type: "noul", instructions: "Is it urgent?" } } });
  });

  it("refuses to send questions a model would refuse, and a decision model without questions", async () => {
    signedIn();
    on("GET", MODELS, page([jev]));
    const { user } = await open("typesafe/jev-1.14.0");
    await screen.findByRole("textbox", { name: "Prompt" });
    await send(user, "Hello");
    expect((await screen.findAllByText("Add a question for Jev 1.14 to answer."))[0]).toBeInTheDocument();
    expect(await screen.findByRole("dialog", { name: "Questions" })).toBeInTheDocument();
    expect(requests("POST", DECISIONS)).toHaveLength(0);
  });

  it("names two questions with the same name before sending", async () => {
    signedIn();
    seed([QUESTIONS[0] as DecisionQuestion, { id: "c", kind: "predicate", name: "refund" }]);
    on("GET", MODELS, page([jev]));
    const { user } = await open("typesafe/jev-1.14.0");
    await screen.findByRole("textbox", { name: "Prompt" });
    await send(user, "Hello");
    expect((await screen.findAllByText("Two questions are named refund; rename one."))[0]).toBeInTheDocument();
    expect(requests("POST", DECISIONS)).toHaveLength(0);
  });

  it("lets a model that also chats chat until there are questions, then asks it them", async () => {
    signedIn();
    on("GET", MODELS, page([luna]));
    on("POST", RESPONSES, () => reply("Hi there."));
    on("POST", DECISIONS, () => decided({ model: "gpt-6-luna" }));
    const { user } = await open("openai/gpt-6-luna");
    expect(await screen.findByRole("textbox", { name: "Prompt" })).toHaveAttribute("placeholder", "Send a prompt");
    await send(user, "Hello");
    expect(await screen.findByText("Hi there.")).toBeInTheDocument();
    expect(requests("POST", DECISIONS)).toHaveLength(0);

    await user.click(screen.getByRole("button", { name: "Questions" }));
    const sheet = await screen.findByRole("dialog", { name: "Questions" });
    await user.click(within(sheet).getByRole("button", { name: "Add question" }));
    await user.click(within(sheet).getByRole("combobox", { name: "Type" }));
    expect(screen.queryByRole("option", { name: "Box" })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    await user.type(within(sheet).getByRole("textbox", { name: "Name" }), "refund");
    expect(screen.getByRole("textbox", { name: "Prompt" })).toHaveAttribute("placeholder", "Describe what the questions are about");
    await send(user, "Refund me");
    await waitFor(() => expect(requests("POST", DECISIONS)).toHaveLength(1));
    expect(requests("POST", DECISIONS)[0]?.body).toMatchObject({ model: "gpt-6-luna", state: "Refund me", questions: { refund: { type: "noul" } } });
    expect(requests("POST", RESPONSES)).toHaveLength(1);
  });

  it("sends attached images as assets and draws a box answer on the image", async () => {
    signedIn();
    seed([{ id: "d", kind: "box", name: "door", instructions: "Where is the door?" }]);
    on("GET", MODELS, page([jev]));
    on("POST", DECISIONS, () =>
      json({ model: "jev-1.14.0", answers: { door: { type: "box", box: { top: 0.2, bottom: 0.6, left: 0.1, right: 0.5 }, confidence: 0.9 } }, usage: {} }),
    );
    const { user } = await open("typesafe/jev-1.14.0");
    await screen.findByRole("button", { name: "Attach images" });
    expect(document.querySelector("input[type=file]")).toHaveAttribute("accept", "image/png,image/jpeg");
    const png = Uint8Array.from(atob(PNG), (c) => c.charCodeAt(0));
    await user.upload(document.querySelector("input[type=file]") as HTMLInputElement, new File([png], "porch.png", { type: "image/png" }));
    await screen.findByRole("list", { name: "Attachments" });
    await send(user, "A delivery photo");
    const answer = await screen.findByRole("article", { name: "Jev 1.14 response" });
    await waitFor(() => expect(within(answer).getByText("20%")).toBeInTheDocument());
    expect(answer.querySelector("[data-box]")).not.toBeNull();
    expect(within(answer).getByRole("img", { name: "porch.png" })).toBeInTheDocument();
    const body = requests("POST", DECISIONS)[0]?.body as { state: unknown; assets: Record<string, { format: string; data_base64: string }> };
    expect(body.state).toEqual(["A delivery photo", "<image-1>"]);
    expect(body.assets["<image-1>"]).toEqual({ type: "image", format: "png", data_base64: PNG });
  });

  it("shows the gateway's refusal in the column and a refused question in its row", async () => {
    signedIn();
    seed(QUESTIONS);
    on("GET", MODELS, page([jev, luna]));
    on("POST", DECISIONS, ({ body }: { body: unknown }) =>
      (body as { model: string }).model === "gpt-6-luna"
        ? apiError(400, "invalid_request", "The model `gpt-6-luna` does not answer box questions.", { param: "questions" })
        : decided({ answers: { refund: { type: "refusal" }, team: answers.team } }),
    );
    const { user } = await open("typesafe/jev-1.14.0,openai/gpt-6-luna");
    await screen.findByRole("textbox", { name: "Prompt" });
    await send(user, "Hello");
    const refused = await screen.findByRole("article", { name: "GPT-6 Luna response" });
    await waitFor(() => expect(refused).toHaveTextContent("The model gpt-6-luna does not answer box questions."));
    const jevAnswer = screen.getByRole("article", { name: "Jev 1.14 response" });
    expect(await within(jevAnswer).findByText("Refused: the model declined to answer this question")).toBeInTheDocument();
    await user.click(within(refused).getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(requests("POST", DECISIONS)).toHaveLength(3));
  });

  it("says when the answers can't be read", async () => {
    signedIn();
    seed(QUESTIONS);
    on("GET", MODELS, page([jev, { ...jev, id: "cm_j2", model: "jev-1.13.0", model_id: "jev-1.13.0", display_name: "Jev 1.13" }]));
    on("POST", DECISIONS, ({ body }: { body: unknown }) =>
      (body as { model: string }).model === "jev-1.13.0" ? new Response("not json", { status: 200 }) : json({ model: "jev-1.14.0", usage: {} }),
    );
    const { user } = await open("typesafe/jev-1.14.0,typesafe/jev-1.13.0");
    await screen.findByRole("textbox", { name: "Prompt" });
    await send(user, "Hello");
    for (const name of ["Jev 1.14 response", "Jev 1.13 response"]) {
      const answer = await screen.findByRole("article", { name });
      await waitFor(() => expect(answer).toHaveTextContent("The model's answers couldn't be read. Try again."));
    }
  });
});
