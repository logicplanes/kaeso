import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, screen, waitFor, within } from "@testing-library/react";
import type { PlaygroundParameters } from "@logicplanes/kaeso-ui/playground";
import type { PlaygroundModel } from "~/api/queries/playground";
import { ORG, PROJECT, signedIn } from "~/test/fixtures";
import { apiError, json, noContent, on, page, requests } from "~/test/network";
import { renderApp } from "~/test/render";
import { playgroundModel } from "~/test/fixtures/project-logs";
import { fileOf, fileRules, type TurnFile } from "./attachments";
import { mediaRun, videoPolling } from "./media";
import { imagesExpected, panelOptions } from "./modes";
import { newResponse } from "./responses";
import { useRecorder } from "./use-recorder";

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
  image: { edits: true, max_count: 4, qualities: ["auto", "high"], size: "pixels" },
});
const speech = playgroundModel({ id: "cm_tts", model: "tts-1", model_id: "tts-1", display_name: "TTS 1", mode: "speech", voices: ["alloy", "nova"] });
const music = playgroundModel({ id: "cm_music", provider: "elevenlabs", model: "music_v1", model_id: "music_v1", display_name: "Eleven Music", mode: "music" });
const whisper = playgroundModel({
  id: "cm_stt",
  model: "whisper-1",
  model_id: "whisper-1",
  display_name: "Whisper",
  mode: "transcription",
  input_modalities: ["audio"],
});
const video = playgroundModel({
  id: "cm_vid",
  provider: "xai",
  model: "grok-imagine-video",
  model_id: "grok-imagine-video",
  display_name: "Grok",
  mode: "video",
});

const none = {} as PlaygroundParameters;
const ctx = { org: ORG, project: PROJECT, now: () => 7 };
const made = (m: PlaygroundModel, prompt: string, files: TurnFile[] = [], p: PlaygroundParameters = none, signal = new AbortController().signal) =>
  mediaRun(ctx, m, prompt, files, p)(signal, vi.fn(), newResponse(0));
const sound = (type: string) => new Response(new Blob([new Uint8Array(4)], { type }));
const clip: TurnFile = { id: "f1", name: "standup.mp3", type: "audio/mpeg", data: "data:audio/mpeg;base64,AAAA" };

beforeEach(() => {
  URL.createObjectURL = vi.fn(() => "blob:kaeso/audio");
  videoPolling.first = 0;
  videoPolling.longest = 0;
});
afterEach(() => {
  videoPolling.first = 2_000;
  videoPolling.longest = 15_000;
});

describe("what a media turn sends and shows", () => {
  it("sends an image quality the model offers, leaves out one it doesn't, and never asks for more images than it makes", async () => {
    on("POST", route("images/generations"), () => json({ data: [{ b64_json: PNG, revised_prompt: "A dock at dawn" }], output_format: "webp" }));
    const state = await made(image, "A dock", [], { ...none, imageQuality: "high", imageCount: 9 });
    expect(state.media).toEqual([{ src: `data:image/webp;base64,${PNG}`, name: "image-1.webp", label: "Image 1", alt: "A dock at dawn" }]);
    await made(image, "A dock", [], { ...none, imageQuality: "hd" });
    await made({ ...image, image: null }, "A dock", [], { ...none, imageCount: 3, imageSize: "1536x1024" });
    expect(requests("POST", route("images/generations")).map((r) => r.body)).toEqual([
      { model: "gpt-image-2", prompt: "A dock", n: 4, quality: "high" },
      { model: "gpt-image-2", prompt: "A dock" },
      { model: "gpt-image-2", prompt: "A dock" },
    ]);
    expect(imagesExpected(image, { ...none, imageCount: 9 })).toBe(4);
    expect(imagesExpected({ ...image, image: null }, { ...none, imageCount: 9 })).toBe(1);
    expect(imagesExpected(image, none)).toBe(1);
    expect(imagesExpected(null, none)).toBe(1);
  });

  it("shows nothing for an image reply it can't read", async () => {
    for (const reply of [new Response("upstream said no"), json([1, 2]), json({ data: "none" })]) {
      on("POST", route("images/generations"), () => reply);
      const state = await made(image, "A dock");
      expect(state.status).toBe("completed");
      expect(state.media).toEqual([]);
      expect(state.usage).toBeNull();
    }
  });

  it("names an audio file by what the provider sent, and by MP3 when it doesn't say", async () => {
    on("POST", route("music"), () => sound("audio/wav"));
    expect((await made(music, "A slow waltz")).media).toEqual([{ src: "blob:kaeso/audio", name: "music.wav", label: "Music" }]);
    expect(requests("POST", route("music"))[0]?.body).toEqual({ model_id: "music_v1", prompt: "A slow waltz" });
    on("POST", route("audio/speech"), () => noContent());
    expect((await made(speech, "Hello", [], { ...none, voice: "echo" })).media?.[0]?.name).toBe("speech.mp3");
    on("POST", route("audio/speech"), () => sound("audio"));
    expect((await made({ ...speech, voices: [] }, "Hello")).media?.[0]?.name).toBe("speech.mp3");
    expect(requests("POST", route("audio/speech")).map((r) => r.body)).toEqual([
      { model: "tts-1", input: "Hello", response_format: "mp3", voice: "alloy" },
      { model: "tts-1", input: "Hello", response_format: "mp3" },
    ]);
  });

  it("sends a note with the audio to transcribe", async () => {
    on("POST", route("audio/transcriptions"), () => json({ text: "Ship it on Monday.", usage: { input_tokens: 12, output_tokens: 5 } }));
    const state = await made(whisper, "Names: Ada, Grace", [clip], { ...none, translate: true });
    expect(state.text).toBe("Ship it on Monday.");
    expect(state.usage).toMatchObject({ input_tokens: 12, output_tokens: 5 });
    const body = String(requests("POST", route("audio/transcriptions"))[0]?.body);
    expect(body).toContain("Names: Ada, Grace");
    expect(body).toContain('filename="standup.mp3"');
  });

  it("shows a video that is done at once without a cost, and one whose cost the gateway doesn't know", async () => {
    on("POST", route("videos"), () => json({ id: "video_1", status: "completed", progress: "done" }));
    const now = await made(video, "A paper plane");
    expect(now.media?.[0]).toMatchObject({ name: "video_1.mp4", label: "Video" });
    expect(now.progress).toBe(100);
    expect(now.cost).toBeUndefined();

    on("POST", route("videos"), () => json({ id: "video_2", status: "queued" }));
    let asked = 0;
    on("GET", route("videos/:video"), () => (++asked < 2 ? json([]) : json({ id: "video_2", status: "completed" }, { headers: { "kaeso-cost": "unknown" } })));
    const later = await made(video, "A paper plane");
    expect(later.status).toBe("completed");
    expect(later.cost).toBeNull();
    expect(later.costKnown).toBe(false);
    expect(asked).toBe(2);
  });

  it("says why a video failed, in the provider's words when it gives any", async () => {
    on("POST", route("videos"), () => json({ id: "video_1", status: "failed", error: { message: "The prompt was refused." } }));
    expect(await made(video, "A paper plane")).toMatchObject({ status: "failed", error: "The prompt was refused.", endedAt: 7 });
    on("POST", route("videos"), () => json({ id: "video_1", status: "failed" }));
    expect((await made(video, "A paper plane")).error).toBe("The provider couldn't make the video. Try again.");
  });

  it("deletes a video only when it was cancelled, and gives up quietly when that fails", async () => {
    on("POST", route("videos"), () => json({ id: "video_1", status: "queued" }));
    on("GET", route("videos/:video"), () => apiError(429, "rate_limited", "Rate limit reached."));
    await expect(made(video, "A paper plane")).rejects.toMatchObject({ status: 429 });
    expect(requests("DELETE", route("videos/:video"))).toHaveLength(0);

    const controller = new AbortController();
    on("GET", route("videos/:video"), () => {
      controller.abort();
      return json({ id: "video_1", status: "in_progress" });
    });
    on("DELETE", route("videos/:video"), () => apiError(500, "internal", "Kaeso failed."));
    await expect(made(video, "A paper plane", [], none, controller.signal)).rejects.toBeDefined();
    await waitFor(() => expect(requests("DELETE", route("videos/:video"))).toHaveLength(1));
  });

  it("offers each parameter only for the models that take it", () => {
    expect(panelOptions([{ ...speech, voices: [] }, speech]).voices).toBe(true);
    expect(panelOptions([speech]).voices).toEqual(["alloy", "nova"]);
    expect(panelOptions([{ ...image, image: null }])).toMatchObject({ text: false, voices: false, duration: false, translate: false });
    expect(panelOptions([{ ...image, image: null }]).image).toBeUndefined();
  });

  it("takes a microphone recording as audio, and no file for an image model that can't edit", () => {
    const rules = fileRules([whisper, { ...image, image: { edits: false, max_count: 1, qualities: [], size: "none" } }]);
    expect(rules.types).toEqual(["audio/*"]);
    expect(["audio/ogg", "video/webm", "video/mp4"].map(rules.takes)).toEqual([true, true, true]);
    expect(["video/quicktime", "image/png"].map(rules.takes)).toEqual([false, false]);
    expect(fileOf({ id: "f", name: "note.ogg", type: "audio/ogg", data: "AAAA" }).type).toBe("audio/ogg");
  });
});

describe("a media turn that fails or is stopped", () => {
  it("shows the gateway's message, with the request id when Kaeso itself failed, and tries again on Retry", async () => {
    signedIn();
    on("GET", MODELS, page([speech]));
    on("POST", route("audio/speech"), () => apiError(400, "invalid_request", "The voice is not one this model speaks."));
    const { user } = await renderApp(`${PLAYGROUND}?models=openai/tts-1`);
    await user.type(await screen.findByRole("textbox", { name: "Prompt" }), "Hello");
    await user.click(screen.getByRole("button", { name: "Send" }));
    const answer = await screen.findByRole("article", { name: "TTS 1 response" });
    expect(await within(answer).findByText("The voice is not one this model speaks.")).toBeInTheDocument();

    on(
      "POST",
      route("audio/speech"),
      () =>
        new Response(JSON.stringify({ error: { type: "internal", message: "Kaeso failed." } }), {
          status: 500,
          headers: { "content-type": "application/json", "kaeso-request-id": "req_42" },
        }),
    );
    await user.click(within(answer).getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Kaeso failed. Request req_42")).toBeInTheDocument();

    on("POST", route("audio/speech"), () => sound("audio/mpeg"));
    vi.mocked(URL.createObjectURL).mockImplementationOnce(() => {
      throw new Error("no room");
    });
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("The connection closed before the reply finished. Try again.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(screen.getByRole("link", { name: "Download Speech" })).toBeInTheDocument());
  });

  it("sends an edit's image again on Retry", async () => {
    signedIn();
    on("GET", MODELS, page([image]));
    on("POST", route("images/edits"), () => apiError(400, "invalid_request", "The image is too small to edit."));
    const { user } = await renderApp(`${PLAYGROUND}?models=openai/gpt-image-2`);
    await screen.findByRole("button", { name: "Attach images" });
    await user.upload(document.querySelector("input[type=file]") as HTMLInputElement, new File([new Uint8Array(8)], "dock.png", { type: "image/png" }));
    await within(composer()).findByRole("list", { name: "Attachments" });
    await user.type(screen.getByRole("textbox", { name: "Prompt" }), "Make it night");
    await user.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByText("The image is too small to edit.")).toBeInTheDocument();
    on("POST", route("images/edits"), () => json({ data: [{ b64_json: PNG }] }));
    await user.click(screen.getByRole("button", { name: "Retry" }));
    const answer = await screen.findByRole("article", { name: "GPT Image 2 response" });
    await waitFor(() => expect(within(answer).getByRole("link", { name: "Download Image 1" })).toBeInTheDocument());
    const bodies = requests("POST", route("images/edits")).map((r) => String(r.body));
    expect(bodies).toHaveLength(2);
    expect(bodies[1]).toContain('filename="dock.png"');
  });

  it("offers to attach files when the chosen models read audio and images", async () => {
    signedIn();
    on("GET", MODELS, page([whisper, image]));
    await renderApp(`${PLAYGROUND}?models=openai/whisper-1,openai/gpt-image-2`);
    expect(await screen.findByRole("button", { name: "Attach files" })).toBeInTheDocument();
  });
});

describe("recording the microphone", () => {
  const record = async (mimeType: string, chunks: Blob[]) => {
    const stopTrack = vi.fn();
    class Recorder {
      mimeType = mimeType;
      ondataavailable: ((e: { data: Blob }) => void) | null = null;
      onstop: (() => void) | null = null;
      start() {}
      stop() {
        for (const data of chunks) this.ondataavailable?.({ data });
        this.onstop?.();
      }
    }
    vi.stubGlobal("MediaRecorder", Recorder);
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getUserMedia: vi.fn().mockResolvedValue({ getTracks: () => [{ stop: stopTrack }] }) },
    });
    const recorded = vi.fn<(file: File) => void>();
    const { result } = renderHook(() => useRecorder(recorded, vi.fn()));
    await act(() => result.current.start());
    expect(result.current.recording).toBe(true);
    act(() => void result.current.stop());
    expect(result.current.recording).toBe(false);
    expect(stopTrack).toHaveBeenCalled();
    vi.unstubAllGlobals();
    return recorded.mock.calls[0]?.[0];
  };
  const bytes = (type: string, size = 8) => new Blob([new Uint8Array(size)], { type });

  it("names the recording by the container the browser recorded", async () => {
    const mp4 = await record("audio/mp4;codecs=mp4a", [bytes("audio/mp4")]);
    expect(mp4?.name).toMatch(/^recording-\d{6}\.m4a$/);
    expect(mp4?.type).toBe("audio/mp4");
    expect((await record("", [bytes("audio/ogg")]))?.name).toMatch(/\.ogg$/);
    expect((await record("", [bytes("")]))?.type).toBe("audio/webm");
  });

  it("hands over nothing when the browser recorded nothing", async () => {
    expect(await record("audio/webm", [])).toBeUndefined();
    expect(await record("", [bytes("audio/webm", 0)])).toBeUndefined();
  });
});
