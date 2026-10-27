import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { ConversationView } from "./conversation.js";

describe("ConversationView", () => {
  it("keeps text plain by default", () => {
    const { container } = render(<ConversationView messages={[{ role: "user", content: "Use **bold** <b>here</b>" }]} />);
    expect(container.querySelector("strong, b")).toBeNull();
    expect(container.textContent).toContain("Use **bold** <b>here</b>");
  });

  it("draws Markdown when asked, loading the renderer on first use, never HTML", async () => {
    const { container } = render(
      <ConversationView
        markdown
        messages={[
          { role: "user", content: "Use **bold** <b>here</b> <img src=x onerror=alert(1)>" },
          {
            role: "assistant",
            content: [
              { type: "reasoning", text: "Thinking about *it*" },
              { type: "tool_result", content: '{"ok":true}' },
            ],
          },
        ]}
      />,
    );
    await act(() => vi.dynamicImportSettled());
    expect(screen.getByText("bold")).toHaveProperty("tagName", "STRONG");
    expect(container.querySelector("b, img")).toBeNull();
    expect(container.textContent).toContain("<b>here</b>");
    expect(container.querySelector("pre")).toBeNull();
    expect(screen.getByText("true")).toBeInTheDocument();
  });

  it("shows inline images, links remote ones without loading them and drops other schemes", () => {
    const pixel = "data:image/png;base64,iVBORw0KGgo=";
    const { container } = render(
      <ConversationView
        messages={[
          {
            role: "user",
            content: [
              { type: "image", src: "javascript:alert(1)", alt: "bad" },
              { type: "image", src: "https://example.com/receipt.png", alt: "Receipt" },
              { type: "image", src: pixel, alt: "Inline" },
            ],
          },
        ]}
      />,
    );
    const links = [...container.querySelectorAll("a")];
    expect(links.map((a) => a.getAttribute("href"))).toEqual(["https://example.com/receipt.png"]);
    expect(links[0]).toHaveTextContent("Image from example.com");
    expect(links[0]?.getAttribute("rel")).toContain("nofollow");
    expect([...container.querySelectorAll("img")].map((i) => i.getAttribute("src"))).toEqual([pixel]);
    expect(container.innerHTML).not.toContain("javascript:");
  });
});
