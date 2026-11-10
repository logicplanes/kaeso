import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { signedIn } from "~/test/fixtures";
import { projectData, projectPath } from "~/test/fixtures/project";
import { logContent, logEntry, logRow, logsData } from "~/test/fixtures/project-logs";
import { renderApp } from "~/test/render";
import Conversation from "./conversation";
import { KeptMedia, MediaList } from "./kept-media";
import { leftOutLabel, leftOutOf } from "./left-out";
import { collectMedia, fileName, forReading, keptOf, shownAs, toBlob, type Kept } from "./logged-media";
import { isKnownReplyForm, replyView, requestView } from "./render";

const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
const WAV = "UklGRkQfAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YSAfAACA";
const MP3 = "SUQzBAAAAAAAI1RTU0U=";
const MP4 = btoa("\0\0\0\x18ftypmp42\0\0\0\0mp42isom");
const WEBM = btoa("\x1a\x45\xdf\xa3 a short recording");
const SVG = btoa('<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>');
const HTML = btoa("<!doctype html><script>alert(1)</script>");

const kept = (base64: string, mediaType: string): Kept => {
  const media = keptOf(base64, mediaType);
  if (!media) throw new Error("not media");
  return media;
};

const made: Blob[] = [];
beforeEach(() => {
  made.length = 0;
  URL.createObjectURL = vi.fn((blob: Blob) => `blob:kaeso/${made.push(blob)}`);
  URL.revokeObjectURL = vi.fn();
});
afterEach(() => vi.restoreAllMocks());

describe("kept media", () => {
  it("is read from a data URL or from base64 beside its type, and nothing else", () => {
    expect(keptOf(`data:image/png;base64,${PNG}`)).toEqual({ mediaType: "image/png", base64: PNG, bytes: 70 });
    expect(keptOf(`data:audio/wav;codecs=1;base64,${WAV}`)?.mediaType).toBe("audio/wav");
    expect(keptOf(PNG, "image/png")?.bytes).toBe(70);
    expect(keptOf(PNG, "")?.mediaType).toBe("application/octet-stream");
    expect(keptOf(PNG)).toBeNull();
    expect(keptOf("https://example.com/cat.png", "image/png")).toBeNull();
    expect(keptOf("data:text/plain,hello")).toBeNull();
    expect(keptOf("AAAA", "image/png")).toBeNull();
    expect(keptOf(7, "image/png")).toBeNull();
  });

  it("is drawn or played by what its bytes are, whatever type the request claimed", () => {
    expect(shownAs(kept(PNG, "image/png"))).toEqual({ kind: "image", type: "image/png", extension: "png" });
    expect(shownAs(kept(PNG, "text/html"))).toMatchObject({ kind: "image", type: "image/png" });
    expect(shownAs(kept(WAV, "audio/wav"))).toMatchObject({ kind: "audio", type: "audio/wav" });
    expect(shownAs(kept(MP3, "audio/mpeg"))).toMatchObject({ kind: "audio", type: "audio/mpeg" });
    expect(shownAs(kept(MP4, "video/mp4"))).toMatchObject({ kind: "video", type: "video/mp4" });
    expect(shownAs(kept(MP4, "audio/mp4"))).toMatchObject({ kind: "audio", type: "audio/mp4" });
    expect(shownAs(kept(WEBM, "audio/webm"))).toMatchObject({ kind: "audio", type: "audio/webm" });
    const download = { kind: "file", type: "application/octet-stream", extension: "bin" };
    expect(shownAs(kept(SVG, "image/svg+xml"))).toEqual(download);
    expect(shownAs(kept(SVG, "image/png"))).toEqual(download);
    expect(shownAs(kept(HTML, "image/png"))).toEqual(download);
    expect(shownAs(kept(btoa("%PDF-1.7 a document"), "application/pdf"))).toEqual({ ...download, extension: "pdf" });
    for (const start of ["\xff\xd8\xff\xe0 a photo..", "GIF89a an animation", "RIFF\0\0\0\0WEBPVP8 ....", "\0\0\0\x18ftypavif\0\0\0\0...."])
      expect(shownAs(kept(btoa(start), "application/octet-stream")).kind).toBe("image");
    for (const start of ["OggS a recording....", "fLaC a recording....", "\xff\xfb\x90\0 a recording", "\0\0\0\x18ftypM4A \0\0\0\0...."])
      expect(shownAs(kept(btoa(start), "application/octet-stream")).kind).toBe("audio");
    expect(shownAs(kept(btoa("\0\0\0\x18ftypqt  \0\0\0\0...."), "video/quicktime")).type).toBe("video/quicktime");
    expect(shownAs(kept(btoa("\0\0\0\x18ftypqt  \0\0\0\0...."), "audio/quicktime"))).toEqual({ kind: "audio", type: "audio/quicktime", extension: "mov" });
    for (const second of ["\xfa", "\xf3", "\xf2", "\xe3"])
      expect(shownAs(kept(btoa(`\xff${second}\x90\0 a recording..`), "audio/mpeg")).type).toBe("audio/mpeg");
    expect(shownAs(kept(btoa("\xff\x00\x90\0 not a recording."), "audio/mpeg")).kind).toBe("file");
    expect(shownAs({ mediaType: "audio/mpeg", base64: "/w==", bytes: 1 }).kind).toBe("file");
    expect(shownAs(kept(btoa("RIFF\0\0\0\0AVI LIST...."), "video/avi")).kind).toBe("file");
    for (const brand of ["heic", "heix", "mif1"])
      expect(shownAs(kept(btoa(`\0\0\0\x18ftyp${brand}\0\0\0\0....`), "image/heic"))).toEqual({
        kind: "file",
        type: "application/octet-stream",
        extension: "heic",
      });
  });

  it("gets a download name from its place and what it is", () => {
    const png = kept(PNG, "image/png");
    expect(fileName(shownAs(png), undefined, 0)).toBe("image-1.png");
    expect(fileName(shownAs(png), "receipt.png", 3)).toBe("receipt.png");
    const mp3 = kept(MP3, "audio/mpeg");
    expect(fileName(shownAs(mp3), undefined, 1)).toBe("audio-2.mp3");
    const pdf = kept(btoa("%PDF-1.7 a document"), "application/pdf");
    expect(fileName(shownAs(pdf), undefined, 2)).toBe("file-3.pdf");
    const svg = kept(SVG, "image/svg+xml");
    expect(fileName(shownAs(svg), undefined, 0)).toBe("file-1.bin");
    expect(toBlob(svg, "application/octet-stream").type).toBe("application/octet-stream");
    expect(toBlob(svg, "application/octet-stream").size).toBe(svg.bytes);
  });

  it("never lets a name an app sent choose what a download is", async () => {
    const png = shownAs(kept(PNG, "image/png"));
    const html = shownAs(kept(HTML, "image/png"));
    const jpeg = shownAs(kept(btoa("\xff\xd8\xff\xe0 a photo.."), "image/jpeg"));
    expect(fileName(html, "invoice.html", 0)).toBe("invoice.bin");
    expect(fileName(html, "setup.exe", 0)).toBe("setup.bin");
    expect(fileName(png, "photo.png.exe", 0)).toBe("photo.png.png");
    expect(fileName(png, "evil\u202egnp.exe", 0)).toBe("evilgnp.png");
    expect(fileName(png, "../..\\etc/passwd", 0)).toBe("etcpasswd.png");
    expect(fileName(png, "\u0000\u200f", 4)).toBe("image-5.png");
    expect(fileName(png, "README", 0)).toBe("README.png");
    expect(fileName(jpeg, "Holiday.JPEG", 0)).toBe("Holiday.JPEG");
    render(<KeptMedia media={kept(PNG, "image/png")} name={"re\u202eceipt.html"} />);
    expect(await screen.findByText("receipt.html (PNG, 70 B)")).toBeInTheDocument();
  });

  it("shows text that only looks like base64 as a file instead of failing", async () => {
    expect(keptOf("A".repeat(17), "image/png")).toBeNull();
    expect(keptOf(`data:image/png;base64,${"A".repeat(21)}`)).toBeNull();
    const odd: Kept = { mediaType: "image/png", base64: "A".repeat(17), bytes: 12 };
    expect(shownAs(odd)).toEqual({ kind: "file", type: "application/octet-stream", extension: "bin" });
    expect(toBlob(odd, "image/png").type).toBe("application/octet-stream");
    render(<KeptMedia media={odd} />);
    expect(await screen.findByRole("link", { name: "Download" })).toHaveAttribute("download", "file-1.bin");
    expect(collectMedia({ media_type: "image/png", data: "A".repeat(17) })).toEqual([]);
  });

  it("draws an image sent in the URL-safe alphabet", async () => {
    const safe = btoa("\xff\xd8\xff\xe0\xfb\xff\xfe a photo....").replace(/\+/g, "-").replace(/\//g, "_");
    expect(safe).toMatch(/[-_]/);
    render(<KeptMedia media={kept(safe, "image/jpeg")} />);
    const img = await screen.findByRole("img", { name: "Image" });
    expect(img.getAttribute("src")).toBe(`data:image/jpeg;base64,${safe.replace(/-/g, "+").replace(/_/g, "/").replace(/=+$/, "")}`);
  });

  it("finds a file under any key beside its type, and under the keys that hold bytes", () => {
    const long = "QUJD".repeat(20);
    expect(
      collectMedia({
        resource: { mimeType: "application/x-thing", blob: long, uri: "file:///a" },
        b64_json: long,
        other: { contentType: "audio/pcm", payload: long, id: long, short: "QUJDQUJDQUJDQUJD" },
        signature: long,
      }).map((m) => ("kept" in m ? m.kept.mediaType : "")),
    ).toEqual(["application/x-thing", "application/octet-stream", "audio/pcm"]);
  });

  it("is found anywhere in logged JSON, in order, beside what was left out", () => {
    const tooLarge = { kaeso_media: { media_type: "video/mp4", bytes: 20_971_520, reason: "too_large" } };
    const found = collectMedia({
      prompt: "Put a hat on it",
      images: [{ source: { base64: { media_type: "image/png", data: PNG } } }, { source: { url: "https://example.com/a.png" } }],
      audio: { source: { base64: { media_type: "audio/mpeg", data: MP3 } } },
      notes: { text: { media_type: "text/plain", data: "SGVsbG8gd29ybGQgYWdhaW4=" } },
      note: { type: "text", media_type: "text/plain", data: "SGVsbG8gd29ybGQgYWdhaW4=" },
      video: { source: { base64: { media_type: "video/mp4", data: tooLarge } } },
      inline: `data:audio/wav;base64,${WAV}`,
      count: 3,
    });
    expect(found).toEqual([
      { kept: { mediaType: "image/png", base64: PNG, bytes: 70 } },
      { kept: { mediaType: "audio/mpeg", base64: MP3, bytes: 14 } },
      { leftOut: leftOutOf(tooLarge) },
      { kept: { mediaType: "audio/wav", base64: WAV, bytes: 45 } },
    ]);
    expect(leftOutLabel(found[2] && "leftOut" in found[2] ? found[2].leftOut : (undefined as never))).toBe("Video (MP4, 20.0 MB), too large to keep");
    const named = leftOutOf({ kaeso_media: { media_type: "application/pdf", bytes: 20_971_520, reason: "too_large" } });
    expect(named && leftOutLabel(named, "terms.pdf")).toBe("terms.pdf (PDF, 20.0 MB), too large to keep");
    for (const type of ["image/png", "audio/wav", "application/zip"]) {
      const media = leftOutOf({ kaeso_media: { media_type: type, bytes: 1024, reason: "too_large" } });
      expect(media && leftOutLabel(media)).toMatch(/^(Image|Audio|File) \(.+, 1\.0 KB\), too large to keep$/);
    }
  });

  it("knows the form of a reply that isn't a chat reply", () => {
    expect(isKnownReplyForm({ format: 2, image: { images: [] } })).toBe(true);
    expect(isKnownReplyForm({ format: 2, transcription: { text: "Hi" } })).toBe(true);
    expect(isKnownReplyForm({ format: 2, image: "x" })).toBe(false);
    expect(isKnownReplyForm({ format: 3, image: { images: [] } })).toBe(false);
  });

  it("is shortened where logged JSON is shown as text", () => {
    const long = "iVBORw0K".repeat(100);
    const shown = forReading({ url: `data:image/png;base64,${long}`, data: long, short: PNG, text: "words ".repeat(200), n: [1, null] }) as Record<
      string,
      unknown
    >;
    expect(shown.url).toBe(`data:image/png;base64,${long.slice(0, 24)}… (600 B)`);
    expect(shown.data).toBe(`${long.slice(0, 24)}… (600 B)`);
    expect(shown.short).toBe(PNG);
    expect(shown.text).toBe("words ".repeat(200));
    expect(shown.n).toEqual([1, null]);
  });

  it("shows an image from a data URL and plays audio and video from blob addresses only", async () => {
    const { container, unmount } = render(
      <MediaList
        media={[
          { kept: kept(PNG, "image/png") },
          { kept: kept(WAV, "audio/wav") },
          { kept: kept(MP4, "video/mp4") },
          { kept: kept(SVG, "image/svg+xml") },
          { leftOut: { mediaType: "image/png", bytes: 1024, reason: "not_kept" } },
        ]}
      />,
    );
    expect(await screen.findByText("Audio (WAV, 45 B)")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Image" })).toHaveAttribute("src", `data:image/png;base64,${PNG.replace(/=+$/, "")}`);
    expect(screen.getByText("Image (PNG, 70 B)")).toBeInTheDocument();
    expect(container.querySelector("audio")).toHaveAttribute("src", "blob:kaeso/2");
    expect(container.querySelector("video")).toHaveAttribute("src", "blob:kaeso/3");
    expect(made.map((b) => b.type)).toEqual(["image/png", "audio/wav", "video/mp4", "application/octet-stream"]);
    expect(await screen.findByText(/^File \(SVG, \d+ B\)$/)).toBeInTheDocument();
    expect(container.querySelectorAll("img")).toHaveLength(1);
    expect(screen.getByText("Image (PNG, 1.0 KB), not kept")).toBeInTheDocument();
    const downloads = screen.getAllByRole("link", { name: "Download" });
    expect(downloads.map((a) => a.getAttribute("download"))).toEqual(["image-1.png", "audio-2.wav", "video-3.mp4", "file-4.bin"]);
    expect(downloads.map((a) => a.getAttribute("href"))).toEqual(["blob:kaeso/1", "blob:kaeso/2", "blob:kaeso/3", "blob:kaeso/4"]);
    expect(container.innerHTML.split(PNG.slice(0, 40))).toHaveLength(2);
    unmount();
    expect(URL.revokeObjectURL).toHaveBeenCalledTimes(4);
    expect(render(<MediaList media={[]} />).container).toBeEmptyDOMElement();
  });

  it("sits in the conversation where each app format carried it", async () => {
    const image = { type: "kept", media: { mediaType: "image/png", base64: PNG, bytes: 70 } };
    const chat = requestView("openai_chat", {
      messages: [
        {
          role: "user",
          content: [
            { type: "image_url", image_url: { url: `data:image/png;base64,${PNG}` } },
            { type: "input_audio", input_audio: { data: WAV, format: "wav" } },
            { type: "file", file: { filename: "terms.pdf", file_data: `data:application/pdf;base64,${PNG}` } },
          ],
        },
      ],
    });
    expect(chat?.messages[0]?.content).toEqual([
      image,
      { type: "kept", media: { mediaType: "audio/wav", base64: WAV, bytes: 45 } },
      { type: "kept", media: { mediaType: "application/pdf", base64: PNG, bytes: 70 }, name: "terms.pdf" },
    ]);
    const anthropic = requestView("anthropic_messages", {
      messages: [{ role: "user", content: [{ type: "image", source: { type: "base64", media_type: "image/png", data: PNG } }] }],
    });
    expect(anthropic?.messages[0]?.content).toEqual([image]);
    const untyped = { type: "kept", media: { mediaType: "application/octet-stream", base64: PNG, bytes: 70 } };
    const bare = requestView("anthropic_messages", { messages: [{ role: "user", content: [{ type: "image", source: { type: "base64", data: PNG } }] }] });
    expect(bare?.messages[0]?.content).toEqual([untyped]);
    expect(replyView({ blocks: [{ image: { source: { base64: { data: PNG } } } }] })?.blocks).toEqual([untyped]);
    const gemini = requestView("gemini", { contents: [{ role: "user", parts: [{ inlineData: { mimeType: "image/png", data: PNG } }] }] });
    expect(gemini?.messages[0]?.content).toEqual([image]);
    const reply = replyView({
      blocks: [
        { image: { source: { base64: { media_type: "image/png", data: PNG } } } },
        { audio: { transcript: "Hi there.", source: { base64: { media_type: "audio/wav", data: WAV } } } },
      ],
    });
    expect(reply?.blocks).toEqual([image, { type: "kept", media: { mediaType: "audio/wav", base64: WAV, bytes: 45 }, transcript: "Hi there." }]);

    const { container } = render(
      <Conversation
        messages={[
          { role: "assistant", content: reply?.blocks ?? [] },
          { role: "user", content: [{ type: "left_out", media: { mediaType: "audio/wav", bytes: 45, reason: "too_large" }, transcript: "Hello?" }] },
        ]}
      />,
    );
    expect(await screen.findByText("Audio (WAV, 45 B)")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "View larger: Image" })).toBeInTheDocument();
    expect(container.querySelector("audio")).toHaveAttribute("src", "blob:kaeso/2");
    expect(screen.getByText("Hi there.")).toBeInTheDocument();
    expect(screen.getByText("Audio (WAV, 45 B), too large to keep")).toBeInTheDocument();
    expect(screen.getByText("Hello?")).toBeInTheDocument();
  });

  it("renders nothing until a played file has its address", () => {
    URL.createObjectURL = vi.fn(() => "");
    const { container } = render(<KeptMedia media={kept(WAV, "audio/wav")} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a media request's files above its fields, and the reply's images above the reply", async () => {
    signedIn();
    projectData();
    const request = {
      model: "gpt-image-2",
      operation: "edit",
      prompt: "Put a hat on it",
      images: [{ source: { base64: { media_type: "image/png", data: PNG } } }],
    };
    const reply = {
      format: 2,
      image: {
        model: "gpt-image-2",
        images: [
          {
            source: {
              base64: {
                media_type: "image/png",
                data: { kaeso_media: { media_type: "image/png", bytes: 20_971_520, width: 2048, height: 2048, reason: "too_large" } },
              },
            },
          },
        ],
      },
    };
    logsData({ rows: [logRow()], entries: [logEntry({ endpoint: "images/edits", ingress_dialect: "openai_images", content: logContent(request, reply) })] });
    const { user } = await renderApp(projectPath("logs?log=req_0001"));
    const sheet = () => screen.findByRole("dialog", { name: /^\d{3}/ });
    await user.click(within(await sheet()).getByRole("tab", { name: "Request" }));
    const panel = await within(await sheet()).findByRole("tabpanel");
    expect(await within(panel).findByRole("img", { name: "Image" })).toHaveAttribute("src", `data:image/png;base64,${PNG.replace(/=+$/, "")}`);
    expect(panel).toHaveTextContent("Put a hat on it");
    await user.click(within(await sheet()).getByRole("tab", { name: "Reply" }));
    expect(await within(await sheet()).findByText("Image (PNG, 2,048 × 2,048, 20.0 MB), too large to keep")).toBeInTheDocument();
    await user.click(within(await sheet()).getByRole("tab", { name: "Raw" }));
    expect(await within(await sheet()).findByRole("tabpanel")).toHaveTextContent("Put a hat on it");
  });
});
