import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { notifications } from "~/test/fixtures/org";
import { json, on, page, requests, type ApiRequest } from "~/test/network";
import { renderApp } from "~/test/render";
import { playgroundModel } from "~/test/fixtures/project-logs";
import { videoPolling } from "./media";

// The prompt's own form: a sent turn shows its attachments too, under the same name.
function composer(): HTMLElement {
  return screen.getByRole("textbox", { name: "Prompt" }).closest("form") as HTMLElement;
}

const MODELS = "orgs/:org/projects/:project/playground/models";
const PLAYGROUND = `/orgs/${ORG}/projects/${PROJECT}/playground`;
const route = (rest: string) => `orgs/:org/projects/:project/playground/${rest}`;
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

const image = playgroundModel({
  id: "cm_img",
  model: "gpt-image-2",
  model_id: "gpt-image-2",
  display_name: "GPT Image 2",
  mode: "image",
  input_modalities: ["text", "image"],
  input_price: null,
  output_price: null,
  image: { edits: true, max_count: 10, qualities: ["auto", "low", "medium", "high"], size: "pixels" },
});
const speech = playgroundModel({ id: "cm_tts", model: "tts-1", model_id: "tts-1", display_name: "TTS 1", mode: "speech", voices: ["alloy", "nova"] });
const sound = playgroundModel({
  id: "cm_sfx",
  provider: "elevenlabs",
  model: "eleven_text_to_sound_v2",
  model_id: "eleven_text_to_sound_v2",
  display_name: "Eleven sound effects",
  mode: "sound_effects",
  durations: true,
});
const music = playgroundModel({
  id: "cm_music",
  provider: "elevenlabs",
  model: "music_v1",
  model_id: "music_v1",
  display_name: "Eleven Music",
  mode: "music",
  durations: true,
});
const whisper = playgroundModel({
  id: "cm_stt",
  model: "whisper-1",
  model_id: "whisper-1",
  display_name: "Whisper",
  mode: "transcription",
  input_modalities: ["audio"],
  translations: true,
});
const video = playgroundModel({
  id: "cm_vid",
  provider: "xai",
  model: "grok-imagine-video",
  model_id: "grok-imagine-video",
  display_name: "Grok Imagine Video",
  mode: "video",
});

const audioReply = (cost = "0.0015") =>
  new Response(new Blob([new Uint8Array([0xff, 0xfb, 0x90, 0])], { type: "audio/mpeg" }), { headers: { "kaeso-cost": cost } });
const send = async (user: Awaited<ReturnType<typeof renderApp>>["user"], text: string) => {
  if (text) await user.type(screen.getByRole("textbox", { name: "Prompt" }), text);
  await user.click(screen.getByRole("button", { name: "Send" }));
};
const open = (models: string) => renderApp(`${PLAYGROUND}?models=${models}`);

beforeEach(() => {
  URL.createObjectURL = vi.fn(() => "blob:kaeso/audio");
  videoPolling.first = 0;
  videoPolling.longest = 0;
});
afterEach(() => {
  videoPolling.first = 2_000;
  videoPolling.longest = 15_000;
});

describe("Playground media", () => {
  it("makes images with the size and count chosen, offers each for download and shows what they cost", async () => {
    signedIn();
    on("GET", MODELS, page([image]));
    on("POST", route("images/generations"), () =>
      json({ data: [{ b64_json: PNG }, { b64_json: PNG }], usage: { input_tokens: 9, output_tokens: 544 } }, { headers: { "kaeso-cost": "0.0421" } }),
    );
    const { user } = await open("openai/gpt-image-2");
    await user.click(await screen.findByRole("button", { name: "Parameters" }));
    const panel = await screen.findByRole("dialog", { name: "Parameters" });
    expect(within(panel).queryByRole("spinbutton", { name: "Max output tokens" })).not.toBeInTheDocument();
    await user.click(within(panel).getByRole("combobox", { name: "Images per prompt" }));
    await user.click(await screen.findByRole("option", { name: "2" }));
    await user.click(within(panel).getByRole("combobox", { name: "Image size" }));
    await user.click(await screen.findByRole("option", { name: "Landscape, 1536 × 1024" }));
    await user.keyboard("{Escape}");
    await send(user, "A loading dock at dawn");
    const answer = await screen.findByRole("article", { name: "GPT Image 2 response" });
    await waitFor(() => expect(within(answer).getAllByRole("img")).toHaveLength(2));
    expect(within(answer).getByRole("link", { name: "Download Image 1" })).toHaveAttribute("download", "image-1.png");
    expect(within(answer).getByText("$0.0421")).toBeInTheDocument();
    expect(requests("POST", route("images/generations"))[0]?.body).toEqual({ model: "gpt-image-2", prompt: "A loading dock at dawn", n: 2, size: "1536x1024" });
  });

  it("edits an attached image through the edits upload", async () => {
    signedIn();
    on("GET", MODELS, page([image]));
    on("POST", route("images/edits"), () => json({ data: [{ b64_json: PNG }] }, { headers: { "kaeso-cost": "unknown" } }));
    const { user } = await open("openai/gpt-image-2");
    await screen.findByRole("button", { name: "Attach images" });
    await user.upload(document.querySelector("input[type=file]") as HTMLInputElement, new File([new Uint8Array(4)], "dock.png", { type: "image/png" }));
    await within(composer()).findByRole("list", { name: "Attachments" });
    await send(user, "Add a red crane");
    const answer = await screen.findByRole("article", { name: "GPT Image 2 response" });
    expect(await within(answer).findByText("Unknown")).toBeInTheDocument();
    const sent = String(requests("POST", route("images/edits"))[0]?.body);
    expect(sent).toContain('name="image[]"; filename="dock.png"');
    expect(sent).toContain("Add a red crane");
  });

  it("speaks in the chosen voice, and makes sound effects and music of the length asked for", async () => {
    signedIn();
    on("GET", MODELS, page([speech, sound, music]));
    on("POST", route("audio/speech"), () => audioReply());
    on("POST", route("sound-effects"), () => audioReply());
    on("POST", route("music"), () => audioReply("unknown"));
    const { user } = await open("openai/tts-1,elevenlabs/eleven_text_to_sound_v2,elevenlabs/music_v1");
    await user.click(await screen.findByRole("button", { name: "Parameters" }));
    const panel = await screen.findByRole("dialog", { name: "Parameters" });
    await user.click(within(panel).getByRole("combobox", { name: "Voice" }));
    await user.click(await screen.findByRole("option", { name: "nova" }));
    await user.type(within(panel).getByRole("spinbutton", { name: "Length in seconds" }), "4");
    await user.keyboard("{Escape}");
    await send(user, "A ship's horn in the fog");
    const spoken = await screen.findByRole("article", { name: "TTS 1 response" });
    expect(await within(spoken).findByRole("link", { name: "Download Speech" })).toHaveAttribute("href", "blob:kaeso/audio");
    expect(spoken.querySelector("audio")).toHaveAttribute("src", "blob:kaeso/audio");
    expect(requests("POST", route("audio/speech"))[0]?.body).toEqual({
      model: "tts-1",
      input: "A ship's horn in the fog",
      response_format: "mp3",
      voice: "nova",
    });
    await waitFor(() => expect(requests("POST", route("music"))).toHaveLength(1));
    expect(requests("POST", route("sound-effects"))[0]?.body).toEqual({
      model_id: "eleven_text_to_sound_v2",
      text: "A ship's horn in the fog",
      duration_seconds: 4,
    });
    expect(requests("POST", route("music"))[0]?.body).toEqual({ model_id: "music_v1", prompt: "A ship's horn in the fog", music_length_ms: 4000 });

    const revoke = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
    await waitFor(() => expect(within(spoken).getByRole("button", { name: "Retry" })).toBeInTheDocument());
    await user.click(within(spoken).getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(revoke).toHaveBeenCalledWith("blob:kaeso/audio"));
    revoke.mockClear();
    await waitFor(() => expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(3));
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(revoke).not.toHaveBeenCalled();
    await send(user, "Rain on a tin roof");
    await waitFor(() => expect(revoke).toHaveBeenCalledTimes(3));
    revoke.mockRestore();
  });

  it("leaves out an image the provider only linked to and a length the model doesn't take", async () => {
    signedIn();
    const qwen = playgroundModel({
      id: "cm_q",
      provider: "alibaba",
      model: "qwen-audio-3.1-tts-next",
      model_id: "qwen-audio-3.1-tts-next",
      display_name: "Qwen Audio TTS Next",
      mode: "sound_effects",
    });
    on("GET", MODELS, page([image, qwen]));
    on("POST", route("images/generations"), () => json({ data: [{ url: "https://cdn.example.com/a.png" }] }));
    on("POST", route("sound-effects"), () => audioReply());
    const { user } = await open("openai/gpt-image-2,alibaba/qwen-audio-3.1-tts-next");
    await user.click(await screen.findByRole("button", { name: "Parameters" }));
    const panel = await screen.findByRole("dialog", { name: "Parameters" });
    expect(within(panel).queryByRole("spinbutton", { name: "Length in seconds" })).not.toBeInTheDocument();
    await user.click(within(panel).getByRole("combobox", { name: "Image quality" }));
    expect(await screen.findByRole("option", { name: "High" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await user.keyboard("{Escape}");
    await send(user, "Rain");
    const answer = await screen.findByRole("article", { name: "GPT Image 2 response" });
    expect(await within(answer).findByText("The model returned nothing to show")).toBeInTheDocument();
    expect(answer.querySelector("img")).toBeNull();
    await waitFor(() => expect(requests("POST", route("sound-effects"))[0]?.body).toEqual({ model_id: "qwen-audio-3.1-tts-next", text: "Rain" }));
  });

  it("transcribes or translates attached audio, and says when there is none", async () => {
    signedIn();
    on("GET", MODELS, page([whisper]));
    on("POST", route("audio/transcriptions"), () => json({ text: "The parcel left the dock this morning.", usage: { type: "duration", seconds: 4 } }));
    on("POST", route("audio/translations"), () => json({ text: "The parcel left the dock." }));
    const { user } = await open("openai/whisper-1");
    expect(await screen.findByRole("textbox", { name: "Prompt" })).toHaveAttribute("placeholder", "Attach or record audio, and add a note if it helps");
    await send(user, "Standup");
    expect(await screen.findByText("Attach or record audio for this model to transcribe.")).toBeInTheDocument();

    await user.upload(document.querySelector("input[type=file]") as HTMLInputElement, new File([new Uint8Array(8)], "standup.mp3", { type: "audio/mpeg" }));
    await within(composer()).findByRole("list", { name: "Attachments" });
    await send(user, "");
    expect(await screen.findByText("The parcel left the dock this morning.")).toBeInTheDocument();
    expect(String(requests("POST", route("audio/transcriptions"))[0]?.body)).toContain('filename="standup.mp3"');

    await user.click(screen.getByRole("button", { name: "Parameters" }));
    await user.click(await screen.findByRole("radio", { name: "In English" }));
    await user.keyboard("{Escape}");
    await user.upload(document.querySelector("input[type=file]") as HTMLInputElement, new File([new Uint8Array(8)], "standup.mp3", { type: "audio/mpeg" }));
    await within(composer()).findByRole("list", { name: "Attachments" });
    await send(user, "");
    expect(await screen.findByText("The parcel left the dock.")).toBeInTheDocument();
  });

  it("follows a video job until it is done, and cancels one on request", async () => {
    signedIn();
    on("GET", MODELS, page([video]));
    let polls = 0;
    on("POST", route("videos"), () => json({ id: "video_1", object: "video", status: "queued", progress: 0 }));
    on("GET", route("videos/:video"), ({ params }: ApiRequest) =>
      params.video === "video_1"
        ? json(
            { id: "video_1", status: ++polls < 2 ? "in_progress" : "completed", progress: polls < 2 ? 40 : 100 },
            polls < 2 ? {} : { headers: { "kaeso-cost": "0.5" } },
          )
        : json({ id: params.video, status: "in_progress", progress: 10 }),
    );
    on("DELETE", route("videos/:video"), ({ params }: ApiRequest) => json({ id: params.video, object: "video.deleted", deleted: true }));
    const { user } = await open("xai/grok-imagine-video");
    await send(user, "A paper plane over the harbour");
    const answer = await screen.findByRole("article", { name: "Grok Imagine Video response" });
    await waitFor(() =>
      expect(answer.querySelector("video")).toHaveAttribute("src", `/platform/v1/orgs/${ORG}/projects/${PROJECT}/playground/videos/video_1/content`),
    );
    expect(within(answer).getByRole("link", { name: "Download Video" })).toHaveAttribute("download", "video_1.mp4");
    expect(within(answer).getByText("$0.50")).toBeInTheDocument();

    on("POST", route("videos"), () => json({ id: "video_2", status: "queued", progress: 0 }));
    videoPolling.first = 60_000;
    videoPolling.longest = 60_000;
    await send(user, "Again, at night");
    const running = await screen.findByRole("progressbar");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(await screen.findByText("Stopped before it finished")).toBeInTheDocument();
    expect(running).not.toBeInTheDocument();
    await waitFor(() => expect(requests("DELETE", route("videos/:video")).map((r) => r.params.video)).toEqual(["video_2"]));
  });

  it("names each model's mode in the picker and routes a file only to the models that read it", async () => {
    signedIn();
    const reader = playgroundModel({ input_modalities: ["text", "pdf"] });
    on("GET", MODELS, page([reader, playgroundModel({ id: "cm_t", model_id: "text-only", model: "text-only", display_name: "Text only" }), image]));
    on(
      "POST",
      "orgs/:org/projects/:project/playground/responses",
      () =>
        new Response('event: response.completed\ndata: {"type":"response.completed","response":{}}\n\n', { headers: { "content-type": "text/event-stream" } }),
    );
    const { user } = await open("openai/gpt-5.4-mini,openai/text-only");
    await user.upload(
      (await screen.findByRole("button", { name: "Attach PDFs" })).parentElement?.querySelector("input[type=file]") as HTMLInputElement,
      new File(["%PDF"], "invoice.pdf", { type: "application/pdf" }),
    );
    await send(user, "Total?");
    await waitFor(() => expect(requests("POST", "orgs/:org/projects/:project/playground/responses")).toHaveLength(2));
    const bodies = requests("POST", "orgs/:org/projects/:project/playground/responses").map((r) => r.body as { model: string; input: { content: unknown }[] });
    const of = (model: string) => bodies.find((b) => b.model === model)?.input.at(-1)?.content;
    expect(of("text-only")).toBe("Total?");
    expect(JSON.stringify(of("gpt-5.4-mini"))).toContain("invoice.pdf");

    await user.click(screen.getAllByRole("button", { description: "Model" })[1] as HTMLElement);
    expect(await screen.findByText("Images")).toBeInTheDocument();
  });

  it("opens on a chat model even when a media model comes first in the list", async () => {
    signedIn();
    on("GET", MODELS, page([image, playgroundModel()]));
    const { router } = await renderApp(PLAYGROUND);
    expect(await screen.findByText("gpt-5.4-mini")).toBeInTheDocument();
    await screen.getAllByRole("button", { name: "Add column" })[0]?.click();
    await waitFor(() => expect(router.state.location.search).toEqual({ models: "openai/gpt-5.4-mini,openai/gpt-image-2" }));
  });

  it("records the microphone for a transcription and says when the browser refuses it", async () => {
    const stop = vi.fn();
    class Recorder {
      mimeType = "audio/webm;codecs=opus";
      ondataavailable: ((e: { data: Blob }) => void) | null = null;
      onstop: (() => void) | null = null;
      start() {}
      stop() {
        this.ondataavailable?.({ data: new Blob([new Uint8Array(16)], { type: "audio/webm" }) });
        this.onstop?.();
      }
    }
    vi.stubGlobal("MediaRecorder", Recorder);
    const getUserMedia = vi.fn().mockResolvedValue({ getTracks: () => [{ stop }] });
    Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: { getUserMedia } });
    signedIn();
    on("GET", MODELS, page([whisper]));
    const { user } = await open("openai/whisper-1");
    await user.click(await screen.findByRole("button", { name: "Record audio" }));
    const stopRecording = await screen.findByRole("button", { name: "Stop recording" });
    expect(stopRecording).toHaveAttribute("aria-pressed", "true");
    await user.click(stopRecording);
    const waiting = await within(composer()).findByRole("list", { name: "Attachments" });
    expect(within(waiting).getByText(/^Recording /)).toBeInTheDocument();
    expect(stop).toHaveBeenCalled();

    getUserMedia.mockRejectedValueOnce(new Error("denied"));
    await act(async () => user.click(screen.getByRole("button", { name: "Record audio" })));
    expect(await notifications().findByText("The microphone isn't available; allow it in the browser.")).toBeInTheDocument();
    vi.unstubAllGlobals();
  });
});
