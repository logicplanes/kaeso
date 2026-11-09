import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import Conversation from "./conversation";
import { formatName, leftOutLabel, leftOutOf } from "./left-out";
import { replyView, requestView } from "./render";

const leftOut = (media_type: string, bytes: number, size?: { width: number; height: number }) => ({
  kaeso_media: { media_type, bytes, ...size, reason: "not_kept" },
});
const read = (v: unknown) => {
  const media = leftOutOf(v);
  if (!media) throw new Error("not a placeholder");
  return media;
};
const png = leftOut("image/png", 1_258_291, { width: 1024, height: 768 });

describe("media the log left out", () => {
  it("reads as its kind, format, size and dimensions", () => {
    expect(leftOutLabel(read(png))).toBe("Image (PNG, 1,024 × 768, 1.2 MB), not kept");
    expect(leftOutLabel(read(leftOut("audio/mpeg", 49_152)))).toBe("Audio (MP3, 48.0 KB), not kept");
    expect(leftOutLabel(read(leftOut("video/mp4", 2_097_152)))).toBe("Video (MP4, 2.0 MB), not kept");
    expect(leftOutLabel(read(leftOut("application/octet-stream", 512)))).toBe("File (512 B), not kept");
    expect(leftOutLabel(read(leftOut("application/pdf", 92_160)), "invoice.pdf")).toBe("invoice.pdf (PDF, 90.0 KB), not kept");
    expect(formatName("audio/x-wav; codecs=1")).toBe("WAV");
    expect(formatName("nonsense")).toBe("");
  });

  it("is told apart from anything an app could send", () => {
    expect(leftOutOf("data:image/png;base64,AAAA")).toBeNull();
    expect(leftOutOf({ kaeso_media: "yes" })).toBeNull();
    expect(leftOutOf({ kaeso_media: { bytes: -4, width: 3 } })).toEqual({ mediaType: "application/octet-stream", bytes: 0, reason: "not_kept" });
  });

  it("is found wherever each app format carries media", () => {
    const block = { type: "left_out", media: leftOutOf(png) };
    const chat = requestView("openai_chat", {
      messages: [
        {
          role: "user",
          content: [
            { type: "image_url", image_url: { url: png } },
            { type: "input_audio", input_audio: { data: leftOut("audio/wav", 9), format: "wav" } },
            { type: "file", file: { filename: "a.pdf", file_data: leftOut("application/pdf", 7) } },
            { type: "video_url", video_url: { url: leftOut("video/mp4", 5) } },
          ],
        },
      ],
    });
    expect(chat?.messages[0]?.content).toEqual([
      block,
      { type: "left_out", media: leftOutOf(leftOut("audio/wav", 9)) },
      { type: "left_out", media: leftOutOf(leftOut("application/pdf", 7)), name: "a.pdf" },
      { type: "left_out", media: leftOutOf(leftOut("video/mp4", 5)) },
    ]);
    const anthropic = requestView("anthropic_messages", {
      messages: [
        {
          role: "user",
          content: [
            { type: "image", source: { type: "base64", media_type: "image/png", data: png } },
            { type: "document", title: "Terms", source: { type: "base64", media_type: "application/pdf", data: leftOut("application/pdf", 7) } },
          ],
        },
      ],
    });
    expect(anthropic?.messages[0]?.content).toEqual([block, { type: "left_out", media: leftOutOf(leftOut("application/pdf", 7)), name: "Terms" }]);
    const responses = requestView("openai_responses", {
      input: [
        {
          role: "user",
          content: [
            { type: "input_image", image_url: png },
            { type: "input_file", filename: "a.pdf", file_data: leftOut("application/pdf", 7) },
          ],
        },
      ],
    });
    expect(responses?.messages[0]?.content).toEqual([block, { type: "left_out", media: leftOutOf(leftOut("application/pdf", 7)), name: "a.pdf" }]);
    const gemini = requestView("gemini", { contents: [{ role: "user", parts: [{ inlineData: { mimeType: "image/png", data: png } }] }] });
    expect(gemini?.messages[0]?.content).toEqual([block]);
    const reply = replyView({
      blocks: [
        { image: { source: { base64: { media_type: "image/png", data: png } } } },
        { audio: { source: { base64: { media_type: "audio/wav", data: leftOut("audio/wav", 9) } } } },
      ],
    });
    expect(reply?.blocks).toEqual([block, { type: "left_out", media: leftOutOf(leftOut("audio/wav", 9)) }]);
  });

  it("shows as a quiet line, and an SVG is never drawn", () => {
    const { container } = render(
      <Conversation
        messages={[
          {
            role: "user",
            content: [
              { type: "left_out", media: read(png) },
              { type: "image", src: "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=", media_type: "image/svg+xml" },
            ],
          },
        ]}
      />,
    );
    expect(screen.getByText("Image (PNG, 1,024 × 768, 1.2 MB), not kept")).toBeInTheDocument();
    expect(screen.getByText("Image image/svg+xml")).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
  });
});
