import { createRef } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { LocaleProvider } from "../../i18n/index.js";
import de from "../../i18n/locales/de.js";
import { MediaView } from "./media-view.js";

const PNG = "data:image/png;base64,iVBOR";

describe("MediaView", () => {
  it("shows an image as a thumbnail with its caption and a download under the name given", () => {
    const ref = createRef<HTMLElement>();
    render(<MediaView ref={ref} src={PNG} label="Image" detail="PNG, 1,024 × 768, 1.2 MB" filename="image-1.png" className="mt-8" />);
    expect(ref.current?.tagName).toBe("FIGURE");
    expect(ref.current).toHaveClass("mt-8");
    expect(screen.getByRole("img", { name: "Image" })).toHaveAttribute("src", PNG);
    expect(screen.getByText("Image (PNG, 1,024 × 768, 1.2 MB)")).toBeInTheDocument();
    const download = screen.getByRole("link", { name: "Download" });
    expect(download).toHaveAttribute("href", PNG);
    expect(download).toHaveAttribute("download", "image-1.png");
  });

  it("opens to the whole image in place and closes again", async () => {
    const user = userEvent.setup();
    render(<MediaView src={PNG} label="A receipt" detail="PNG, 48.0 KB" />);
    const open = screen.getByRole("button", { name: "View larger: A receipt" });
    expect(open).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByRole("img", { name: "A receipt" })).toHaveClass("object-cover");
    await user.click(open);
    const close = screen.getByRole("button", { name: "View smaller: A receipt" });
    expect(close).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("img", { name: "A receipt" })).toHaveClass("object-contain", "max-h-480");
    await user.click(close);
    expect(screen.getByRole("img", { name: "A receipt" })).toHaveClass("object-cover");
  });

  it("plays audio and video in the browser's player, named for assistive technology", () => {
    const { container, rerender } = render(<MediaView kind="audio" src="blob:kaeso/1" label="Audio" detail="MP3, 48.0 KB" />);
    const audio = container.querySelector("audio");
    expect(audio).toHaveAttribute("src", "blob:kaeso/1");
    expect(audio).toHaveAttribute("controls");
    expect(audio).toHaveAttribute("aria-label", "Audio");
    expect(audio).toHaveClass("max-w-400");
    expect(screen.queryByRole("button")).toBeNull();
    rerender(<MediaView kind="video" src="blob:kaeso/2" label="Video" />);
    const video = container.querySelector("video");
    expect(video).toHaveAttribute("src", "blob:kaeso/2");
    expect(video).toHaveAttribute("controls");
    expect(video).toHaveClass("rounded-well");
    expect(screen.getByText("Video")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Download" })).toHaveAttribute("download", "");
  });

  it("downloads from another address than the one it shows when given one", () => {
    render(<MediaView src={PNG} download="blob:kaeso/9" label="Image" filename="image-1.png" />);
    expect(screen.getByRole("img", { name: "Image" })).toHaveAttribute("src", PNG);
    expect(screen.getByRole("link", { name: "Download" })).toHaveAttribute("href", "blob:kaeso/9");
  });

  it("offers a file that can't be shown as a download alone", () => {
    const { container } = render(<MediaView kind="file" src="blob:kaeso/3" label="invoice.pdf" detail="PDF, 90.0 KB" filename="invoice.pdf" />);
    expect(container.querySelector("img, audio, video, button")).toBeNull();
    expect(screen.getByText("invoice.pdf (PDF, 90.0 KB)")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Download" })).toHaveAttribute("download", "invoice.pdf");
  });

  it("speaks the provider's language", () => {
    render(
      <LocaleProvider locale="de" messages={de}>
        <MediaView src={PNG} label="Bild" detail="PNG" />
      </LocaleProvider>,
    );
    expect(screen.getByRole("link", { name: "Herunterladen" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Größer ansehen: Bild" })).toBeInTheDocument();
  });
});
