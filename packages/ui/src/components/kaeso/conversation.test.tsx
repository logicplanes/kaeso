import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ConversationView } from "./conversation";

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
    expect(await screen.findByText("bold")).toHaveProperty("tagName", "STRONG");
    expect(container.querySelector("b, img")).toBeNull();
    expect(container.textContent).toContain("<b>here</b>");
    expect(container.querySelector("pre")).toHaveTextContent('{"ok":true}');
  });

  it("opens images only from http(s) and inline data", () => {
    const { container } = render(
      <ConversationView
        messages={[
          {
            role: "user",
            content: [
              { type: "image", src: "javascript:alert(1)", alt: "bad" },
              { type: "image", src: "https://example.com/receipt.png", alt: "Receipt" },
            ],
          },
        ]}
      />,
    );
    const links = [...container.querySelectorAll("a")];
    expect(links.map((a) => a.getAttribute("href"))).toEqual(["https://example.com/receipt.png"]);
    expect(container.innerHTML).not.toContain("javascript:");
  });
});
