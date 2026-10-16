import { fireEvent, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { hoverReason, notifications } from "~/test/fixtures/org";
import { renderUi } from "~/test/render";
import { PictureControl, toPictureUpload } from "./picture";

const picker = () => document.querySelector<HTMLInputElement>('input[type="file"]') as HTMLInputElement;

function canvas(blob: Blob | null, context: unknown = { drawImage: vi.fn(), imageSmoothingQuality: "low" }) {
  const close = vi.fn();
  vi.stubGlobal(
    "createImageBitmap",
    vi.fn(async () => ({ width: 300, height: 500, close })),
  );
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(context as never);
  vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation((done: BlobCallback) => done(blob));
  return { close, context };
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("PictureControl", () => {
  it("uploads a chosen image and ignores other files", async () => {
    const onPick = vi.fn();
    const { user } = await renderUi(<PictureControl name="Ada Okafor" src={null} onPick={onPick} onRemove={vi.fn()} />);

    const browse = vi.spyOn(HTMLInputElement.prototype, "click");
    await user.click(screen.getByRole("button", { name: "Upload" }));
    expect(browse).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
    const png = new File(["png"], "me.png", { type: "image/png" });
    await user.upload(picker(), png);
    expect(onPick).toHaveBeenCalledWith(png);
    expect(picker().value).toBe("");

    await userEvent.setup({ applyAccept: false }).upload(picker(), new File(["gif"], "me.gif", { type: "image/gif" }));
    expect((await notifications().findAllByText("Choose a PNG, JPEG or WebP image.")).length).toBeGreaterThan(0);
    expect(onPick).toHaveBeenCalledTimes(1);
  });

  it("does nothing when the file picker closes without a file", async () => {
    const onPick = vi.fn();
    await renderUi(<PictureControl name="Ada Okafor" src={null} onPick={onPick} onRemove={vi.fn()} />);
    fireEvent.change(picker(), { target: { files: [] } });
    expect(onPick).not.toHaveBeenCalled();
    expect(notifications().queryByText("Choose a PNG, JPEG or WebP image.")).not.toBeInTheDocument();
  });

  it("replaces or removes the current picture", async () => {
    const onRemove = vi.fn();
    const { user } = await renderUi(<PictureControl name="Northwind" kind="org" src="/avatars/org.webp" onPick={vi.fn()} onRemove={onRemove} busy />);
    expect(screen.getByRole("button", { name: "Replace" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(onRemove).toHaveBeenCalled();
  });

  it("explains why the picture can't change", async () => {
    const { user } = await renderUi(
      <PictureControl name="Northwind" src="/avatars/org.webp" onPick={vi.fn()} onRemove={vi.fn()} disabledReason="You need Edit organization." />,
    );
    const replace = screen.getByRole("button", { name: "Replace" });
    expect(replace).toHaveAttribute("aria-disabled", "true");
    expect(screen.queryByRole("button", { name: "Remove" })).not.toBeInTheDocument();
    await hoverReason(user, replace, "You need Edit organization.");
  });
});

describe("toPictureUpload", () => {
  it("crops the middle square to 256 pixels as WebP", async () => {
    const draw = vi.fn();
    const { close } = canvas(new Blob([new Uint8Array([104, 105])], { type: "image/webp" }), { drawImage: draw, imageSmoothingQuality: "low" });
    expect(await toPictureUpload(new File(["x"], "me.png", { type: "image/png" }))).toEqual({ content_type: "image/webp", data: btoa("hi") });
    expect(draw).toHaveBeenCalledWith(expect.anything(), 0, 100, 300, 300, 0, 0, 256, 256);
    expect(close).toHaveBeenCalled();
  });

  it("falls back to PNG when the browser can't write WebP", async () => {
    canvas(new Blob([new Uint8Array([1])], { type: "image/png" }));
    expect(await toPictureUpload(new File(["x"], "me.png", { type: "image/png" }))).toEqual({ content_type: "image/png", data: btoa("\u0001") });
  });

  it("says when the browser can't prepare the picture", async () => {
    canvas(null);
    await expect(toPictureUpload(new File(["x"], "me.png", { type: "image/png" }))).rejects.toThrow("This browser can't prepare the picture.");
    canvas(new Blob(), null);
    await expect(toPictureUpload(new File(["x"], "me.png", { type: "image/png" }))).rejects.toThrow("This browser can't prepare the picture.");
  });
});
