import { describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { eventStream, playgroundModel, sse } from "~/test/fixtures/project-logs";
import { fileRules, userContent } from "./attachments";

const MODELS = "orgs/:org/projects/:project/playground/models";
const RESPONSES = "orgs/:org/projects/:project/playground/responses";
const PLAYGROUND = `/orgs/${ORG}/projects/${PROJECT}/playground`;
const vision = playgroundModel({ input_modalities: ["text", "image", "pdf"] });
const reply = (text: string) =>
  eventStream(
    sse([
      { type: "response.created" },
      { type: "response.output_text.delta", item_id: "m", delta: text },
      { type: "response.completed", response: { usage: { input_tokens: 3, output_tokens: 2 } } },
    ]),
  );
const sentBodies = () => requests("POST", RESPONSES).map((r) => r.body as { input: { role: string; content: unknown }[] });
const picker = () => document.querySelector("input[type=file]") as HTMLInputElement;
const png = (name = "dock.png", bytes = 8) => new File([new Uint8Array(bytes)], name, { type: "image/png" });

describe("Playground attachments", () => {
  it("sends an image with the prompt, shows it above the prompt and sends it again with the next turn", async () => {
    signedIn();
    on("GET", MODELS, page([vision]));
    on("POST", RESPONSES, () => reply("A loading dock."));
    const { user } = await renderApp(PLAYGROUND);
    await user.click(await screen.findByRole("button", { name: "Attach images or PDFs" }));
    expect(picker()).toHaveAttribute("accept", "image/png,image/jpeg,image/webp,image/gif,application/pdf");
    await user.upload(picker(), png());
    const waiting = await screen.findByRole("list", { name: "Attachments" });
    expect(within(waiting).getByRole("img", { name: "dock.png" })).toBeInTheDocument();
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "What is this?");
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText("A loading dock.")).toBeInTheDocument();
    const prompt = screen.getByRole("group", { name: "Prompt" });
    expect(within(prompt).getByRole("img", { name: "dock.png" })).toBeInTheDocument();
    expect(screen.getAllByRole("list", { name: "Attachments" })).toHaveLength(1);
    const [first] = sentBodies();
    expect(first?.input.at(-1)).toEqual({
      role: "user",
      content: [
        { type: "input_text", text: "What is this?" },
        { type: "input_image", image_url: expect.stringMatching(/^data:image\/png;base64,/) },
      ],
    });

    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "And the colour?");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(sentBodies()).toHaveLength(2));
    const history = sentBodies()[1]?.input ?? [];
    expect(history[0]?.content).toEqual(first?.input.at(-1)?.content);
    expect(history.at(-1)).toEqual({ role: "user", content: "And the colour?" });
  });

  it("refuses a file the models don't take and one that makes the conversation too large, keeping the rest", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel({ input_modalities: ["text", "image"], max_request_bytes: 64 * 1024 + 200 })]));
    const { user } = await renderApp(PLAYGROUND);
    const attach = await screen.findByRole("button", { name: "Attach images" });
    expect(attach).toBeEnabled();
    expect(picker()).toHaveAttribute("accept", "image/png,image/jpeg,image/webp,image/gif");
    const form = screen.getByRole("textbox", { name: "Prompt" }).closest("form") as HTMLFormElement;
    const dropped = [new File(["%PDF"], "invoice.pdf", { type: "application/pdf" }), png("small.png"), png("large.png", 400)];
    fireEvent.drop(form, { dataTransfer: { types: ["Files"], files: dropped } });
    expect(await notifications().findByText("2 files were too large or not taken.")).toBeInTheDocument();
    const waiting = screen.getByRole("list", { name: "Attachments" });
    expect(within(waiting).getAllByRole("listitem")).toHaveLength(1);
    await user.click(within(waiting).getByRole("button", { name: "Remove small.png" }));
    expect(screen.queryByRole("list", { name: "Attachments" })).toBeNull();

    await user.upload(picker(), png("huge.png", 400));
    expect(await notifications().findByText("huge.png makes the conversation too large to send.")).toBeInTheDocument();
    const broken = vi.spyOn(FileReader.prototype, "readAsDataURL").mockImplementation(function (this: FileReader) {
      this.onerror?.(new ProgressEvent("error") as ProgressEvent<FileReader>);
    });
    await user.upload(picker(), png("locked.png"));
    expect(await notifications().findByText("locked.png couldn't be read; try it again.")).toBeInTheDocument();
    broken.mockRestore();
  });

  it("counts the files already sent and the ones waiting against the size of the next message", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel({ input_modalities: ["text", "image"], max_request_bytes: 64 * 1024 + 200 })]));
    on("POST", RESPONSES, () => reply("Noted."));
    const { user } = await renderApp(PLAYGROUND);
    await user.type(await screen.findByRole("textbox", { name: "Prompt" }), "Hello");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(sentBodies()).toHaveLength(1));

    await user.upload(picker(), png("first.png", 60));
    await screen.findByRole("list", { name: "Attachments" });
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "What is this?");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await waitFor(() => expect(sentBodies()).toHaveLength(2));
    await waitFor(() => expect(screen.getAllByText("Noted.")).toHaveLength(2));

    await user.upload(picker(), png("second.png", 60));
    expect(await notifications().findByText("second.png makes the conversation too large to send.")).toBeInTheDocument();
    await user.upload(picker(), png("small.png"));
    expect(await screen.findByRole("img", { name: "small.png" })).toBeInTheDocument();
    await user.upload(picker(), png("another.png", 60));
    expect(await notifications().findByText("another.png makes the conversation too large to send.")).toBeInTheDocument();
  });

  it("offers no attachments when no chosen model reads files", async () => {
    signedIn();
    on("GET", MODELS, page([playgroundModel({ id: "cm_t", model_id: "text-only", model: "text-only", display_name: "Text only" })]));
    await renderApp(PLAYGROUND);
    await screen.findByText("text-only");
    expect(screen.queryByRole("button", { name: /^Attach/ })).not.toBeInTheDocument();
  });

  it("sends a PDF as a file and a prompt of files alone without text", () => {
    expect(userContent("", [{ id: "f", name: "invoice.pdf", type: "application/pdf", data: "data:application/pdf;base64,JVBERg==" }])).toEqual([
      { type: "input_file", filename: "invoice.pdf", file_data: "data:application/pdf;base64,JVBERg==" },
    ]);
    expect(userContent("Hi", [])).toBe("Hi");
    const none = fileRules([]);
    expect([none.types, none.takes("image/png"), none.maxBytes("image/png")]).toEqual([[], false, 0]);
  });
});
