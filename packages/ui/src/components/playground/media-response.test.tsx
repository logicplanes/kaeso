import { describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { MediaResponse } from "./media-response.js";
import { ParameterPanel, type PlaygroundParameters } from "./parameter-panel.js";
import { ResponseMetrics } from "./response-metrics.js";
import { Button } from "../button/button.js";

const PIXEL = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";
const footer = <ResponseMetrics duration={2400} cost="0.04" />;

describe("MediaResponse", () => {
  it("holds a square for every image it waits for, then shows each with Download", () => {
    const { container, rerender } = render(<MediaResponse kind="image" status="pending" expected={2} footer={footer} aria-label="GPT Image 2 response" />);
    const article = screen.getByRole("article", { name: "GPT Image 2 response" });
    expect(article).toHaveAttribute("aria-busy", "true");
    expect(container.querySelectorAll("li")).toHaveLength(2);
    expect(screen.getByText("Making the image")).toBeInTheDocument();
    expect(container.querySelector("[data-tools-reserved]")).toHaveClass("invisible");
    rerender(
      <MediaResponse
        kind="image"
        status="done"
        items={[
          { src: PIXEL, name: "image-1.png", alt: "A loading dock at dawn" },
          { src: PIXEL, name: "image-2.png" },
        ]}
        footer={footer}
        onRetry={() => undefined}
        aria-label="GPT Image 2 response"
      />,
    );
    expect(screen.getByRole("img", { name: "A loading dock at dawn" })).toHaveAttribute("src", PIXEL);
    expect(screen.getByRole("img", { name: "image-2.png" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Download image-1.png" })).toHaveAttribute("download", "image-1.png");
    expect(screen.getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("GPT Image 2 response finished");
  });

  it("plays audio, and says when a model returned nothing", () => {
    const { container, rerender } = render(<MediaResponse kind="audio" status="running" />);
    expect(screen.getByText("Making the audio")).toBeInTheDocument();
    rerender(<MediaResponse kind="audio" items={[{ src: "blob:kaeso/1", name: "speech.mp3" }]} />);
    expect(container.querySelector("audio")).toHaveAttribute("src", "blob:kaeso/1");
    expect(screen.getByRole("link", { name: "Download speech.mp3" })).toHaveAttribute("href", "blob:kaeso/1");
    rerender(<MediaResponse kind="audio" items={[]} />);
    expect(screen.getByText("The model returned nothing to show")).toBeInTheDocument();
  });

  it("shows a video job's progress and the seconds it has taken, cancels it, then plays the video", () => {
    vi.useFakeTimers();
    const cancel = vi.fn();
    const { container, rerender } = render(<MediaResponse kind="video" status="pending" onCancel={cancel} />);
    expect(screen.getByRole("progressbar", { name: "Waiting for the provider to start the video" })).not.toHaveAttribute("aria-valuenow");
    rerender(<MediaResponse kind="video" status="running" progress={40} onCancel={cancel} />);
    expect(screen.getByRole("progressbar", { name: "Making the video, 40% done" })).toHaveAttribute("aria-valuenow", "40");
    act(() => vi.advanceTimersByTime(3_000));
    expect(screen.getByText("3 s")).toBeInTheDocument();
    screen.getByRole("button", { name: "Cancel" }).click();
    expect(cancel).toHaveBeenCalled();
    vi.useRealTimers();
    rerender(<MediaResponse kind="video" items={[{ src: "/platform/v1/video_1/content", name: "video_1.mp4" }]} />);
    expect(container.querySelector("video")).toHaveAttribute("src", "/platform/v1/video_1/content");
    expect(screen.getByRole("link", { name: "Download video_1.mp4" })).toBeInTheDocument();
  });

  it("reads a stop and a failure as a text response does, with Retry", async () => {
    const retry = vi.fn();
    const { rerender } = render(<MediaResponse kind="image" status="stopped" onRetry={retry} />);
    expect(screen.getByText("Stopped before it finished")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    rerender(<MediaResponse kind="audio" status="error" error="The provider refused the request as invalid; check the model's settings." onRetry={retry} />);
    expect(screen.getByRole("alert")).toHaveTextContent("No response");
    expect(screen.getByRole("alert")).toHaveTextContent("check the model's settings");
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(retry).toHaveBeenCalledTimes(2);
  });
});

describe("ResponseMetrics without a known cost", () => {
  it("says the cost is unknown once the response is done, never while it runs", () => {
    const { rerender } = render(<ResponseMetrics duration={1200} costKnown={false} />);
    expect(screen.getByText("Unknown")).toBeInTheDocument();
    rerender(<ResponseMetrics live startedAt={Date.now()} costKnown={false} />);
    expect(screen.queryByText("Unknown")).toBeNull();
  });
});

describe("Button download", () => {
  it("saves a link under a name instead of opening it", () => {
    render(
      <Button href="blob:kaeso/1" download="speech.mp3">
        Save
      </Button>,
    );
    expect(screen.getByRole("link", { name: "Save" })).toHaveAttribute("download", "speech.mp3");
  });
});

function Media({ start = {} }: { start?: PlaygroundParameters }) {
  const [value, setValue] = useState<PlaygroundParameters>(start);
  return (
    <>
      <ParameterPanel
        value={value}
        onValueChange={setValue}
        text={false}
        instructions={false}
        sampling={false}
        image={{ maxCount: 10, qualities: ["low", "high"], sizes: true }}
        voices={["alloy", "nova"]}
        duration
        translate
      />
      <output>{JSON.stringify(value)}</output>
    </>
  );
}

describe("ParameterPanel for media models", () => {
  it("sets the image size, count and quality, the voice, the length and the transcript language, and resets them", async () => {
    render(<Media />);
    expect(screen.queryByRole("spinbutton", { name: "Max output tokens" })).toBeNull();
    const pick = async (field: string, option: string) => {
      await userEvent.click(screen.getByRole("combobox", { name: field }));
      await userEvent.click(await screen.findByRole("option", { name: option }));
    };
    await pick("Image size", "Portrait, 1024 × 1536");
    await pick("Images per prompt", "4");
    expect(screen.queryByRole("option", { name: "5" })).toBeNull();
    await pick("Image quality", "high");
    await pick("Voice", "nova");
    await userEvent.type(screen.getByRole("spinbutton", { name: "Length in seconds" }), "12");
    await userEvent.click(screen.getByRole("radio", { name: "In English" }));
    expect(JSON.parse(screen.getByRole("status").textContent ?? "{}")).toEqual({
      imageSize: "1024x1536",
      imageCount: 4,
      imageQuality: "high",
      voice: "nova",
      durationSeconds: 12,
      translate: true,
    });
    await pick("Voice", "alloy");
    await pick("Images per prompt", "1");
    await userEvent.click(screen.getByRole("radio", { name: "As spoken" }));
    await userEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));
    expect(screen.getByRole("status")).toHaveTextContent("{}");
  });

  it("takes any voice name when the provider names none", async () => {
    function Free() {
      const [value, setValue] = useState<PlaygroundParameters>({});
      return (
        <>
          <ParameterPanel value={value} onValueChange={setValue} voices />
          <output>{JSON.stringify(value)}</output>
        </>
      );
    }
    render(<Free />);
    const field = within(document.body).getByRole("textbox", { name: "Voice" });
    expect(field).toHaveAttribute("placeholder", "A voice name or id");
    await userEvent.type(field, "Kore");
    expect(screen.getByRole("status")).toHaveTextContent('"voice":"Kore"');
    await userEvent.clear(field);
    expect(screen.getByRole("status")).toHaveTextContent("{}");
  });
});
