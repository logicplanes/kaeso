import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import Conversation from "./conversation";

describe("Conversation", () => {
  it("draws message text as Markdown and keeps JSON tool results as sent", () => {
    const { container } = render(
      <Conversation
        messages={[
          { role: "system", content: "# Harness\n\n- Read the file\n- Answer" },
          { role: "user", content: [{ type: "text", text: "Where is **order 7**?" }] },
          { role: "tool", content: [{ type: "tool_result", tool_call_id: "call_1", content: '{"status": "*shipped*"}' }] },
          {
            role: "assistant",
            content: [
              { type: "reasoning", text: "Check `orders`." },
              { type: "text", text: "It *shipped*." },
            ],
          },
        ]}
      />,
    );
    const system = screen.getByRole("region", { name: "System" });
    expect(screen.getByRole("heading", { name: "Harness" })).toBeInTheDocument();
    expect(system.querySelectorAll("li")).toHaveLength(2);
    expect(screen.getByRole("region", { name: "User" }).querySelector("strong")).toHaveTextContent("order 7");
    expect(screen.getByRole("region", { name: "Tool" }).querySelector("pre")).toHaveTextContent('{"status": "*shipped*"}');
    expect(screen.getByRole("region", { name: "Assistant" }).querySelector("em")).toHaveTextContent("shipped");
    expect(container.querySelector("img")).toBeNull();
  });

  it("keeps a message's raw HTML as text and links only to http, https and mailto", () => {
    const { container } = render(
      <Conversation
        messages={[
          {
            role: "user",
            content:
              '<img src=x onerror="alert(1)"> <script>alert(2)</script> [docs](https://kaeso.example.com/docs) [bad](javascript:alert(1)) ![chart](https://kaeso.example.com/chart.png)',
          },
        ]}
      />,
    );
    const user = screen.getByRole("region", { name: "User" });
    expect(container.querySelector("img, script, [onerror]")).toBeNull();
    expect(user).toHaveTextContent('<img src=x onerror="alert(1)">');
    expect(screen.getByRole("link", { name: /^docs/ })).toHaveAttribute("href", "https://kaeso.example.com/docs");
    expect(screen.queryByRole("link", { name: /^bad/ })).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: /^chart/ })).toHaveAttribute("rel", "noopener noreferrer nofollow");
  });

  it("says whether reasoning without text was hidden or redacted", () => {
    render(
      <Conversation
        messages={[
          {
            role: "assistant",
            content: [
              { type: "reasoning", text: "" },
              { type: "reasoning", text: "", redacted: true },
            ],
          },
        ]}
      />,
    );
    expect(screen.getByText("Hidden by the display setting")).toBeInTheDocument();
    expect(screen.getByText("Redacted by the provider")).toBeInTheDocument();
  });

  it("draws tool calls, results, media, documents and refusals", () => {
    const { container } = render(
      <Conversation
        messages={[
          {
            role: "assistant",
            content: [
              { type: "tool_call", id: "call_9", name: "lookup_order", arguments: { order: 7 } },
              { type: "tool_call", name: "ping", arguments: {} },
              { type: "refusal", text: "That isn't something I can share." },
            ],
          },
          {
            role: "tool",
            content: [
              { type: "tool_result", tool_call_id: "call_9", is_error: true, content: [{ type: "text", text: "Order **not** found" }] },
              { type: "tool_result", content: "{not json" },
              { type: "tool_result", content: "Delivered _today_" },
              { type: "tool_result", content: "[1, 2]" },
            ],
          },
          {
            role: "user",
            content: [
              { type: "image", src: "data:image/png;base64,iVBOR", alt: "A receipt" },
              { type: "image", src: "data:image/jpeg;base64,/9j/" },
              { type: "image", src: "https://example.com/photo.jpg" },
              { type: "image", src: "javascript:alert(1)", media_type: "image/png" },
              { type: "image" },
              { type: "document", name: "invoice.pdf" },
              { type: "document" },
            ],
          },
          { role: "developer", content: "Stay on topic." },
        ]}
      />,
    );
    const assistant = screen.getByRole("region", { name: "Assistant" });
    expect(assistant).toHaveTextContent("Called lookup_order");
    expect(assistant).toHaveTextContent("call_9");
    expect(assistant).toHaveTextContent("Called ping");
    expect(assistant).toHaveTextContent("The model refused.");
    expect(assistant).toHaveTextContent("That isn't something I can share.");
    const tool = screen.getByRole("region", { name: "Tool" });
    expect(tool).toHaveTextContent("Error");
    expect(tool.querySelector("strong")).toHaveTextContent("not");
    expect(tool).toHaveTextContent("{not json");
    expect(tool.querySelector("em")).toHaveTextContent("today");
    expect([...tool.querySelectorAll("pre")].map((p) => p.textContent)).toEqual(["[1, 2]"]);
    const receipt = screen.getByRole("img", { name: "A receipt" });
    const unnamed = screen.getByRole("img", { name: "Image input" });
    expect(receipt).toHaveAttribute("src", "data:image/png;base64,iVBOR");
    expect(unnamed).toHaveAttribute("src", "data:image/jpeg;base64,/9j/");
    expect(container.querySelectorAll("img")).toHaveLength(2);
    expect(receipt.closest("a")).toBeNull();
    fireEvent.error(receipt);
    expect(screen.getByRole("img", { name: "A receipt" })).toHaveTextContent("A receipt");
    expect(screen.getByRole("link", { name: /Image from example\.com/ })).toHaveAttribute("href", "https://example.com/photo.jpg");
    expect(container.querySelectorAll("a[target=_blank]")).toHaveLength(1);
    const user = screen.getByRole("region", { name: "User" });
    expect(user).toHaveTextContent("Image image/png");
    expect(user).toHaveTextContent("invoice.pdf");
    expect(user).toHaveTextContent("Document");
    expect(container.innerHTML).not.toContain("javascript:");
    expect(screen.getByRole("region", { name: "Developer" })).toHaveTextContent("Stay on topic.");
  });

  it("shows reasoning text when the provider returned it", () => {
    render(<Conversation messages={[{ role: "assistant", content: [{ type: "reasoning", text: "Weigh **both** options." }] }]} />);
    expect(screen.getByText("Reasoning")).toBeInTheDocument();
    expect(screen.queryByText("Hidden by the display setting")).not.toBeInTheDocument();
  });
});
