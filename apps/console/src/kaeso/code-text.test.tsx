import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { notifications } from "~/test/fixtures/org";
import { renderUi } from "~/test/render";
import { toast } from "~/shell/toast";
import { CodeText } from "./code-text";

describe("CodeText", () => {
  it("sets the parts between backticks as code and leaves a lone backtick as it is", () => {
    const { container, rerender } = render(<CodeText text="Groq does not serve the model `llama-3.1-8b-instant` to this key." />);
    expect(screen.getByText("llama-3.1-8b-instant").tagName).toBe("CODE");
    expect(container).toHaveTextContent("Groq does not serve the model llama-3.1-8b-instant to this key.");
    rerender(<CodeText text="Plain words with one ` mark." />);
    expect(container.querySelector("code")).toBeNull();
    expect(container).toHaveTextContent("Plain words with one ` mark.");
  });
});

describe("toast", () => {
  it("shows code in a message's title and description as code", async () => {
    await renderUi(<p>Page</p>);
    act(() => toast.error("`amount` must be above zero.", { description: "Send `amount` as a number." }));
    expect(await notifications().findAllByText("amount", { selector: "code" })).toHaveLength(2);
    act(() => toast.warning("No code here", { description: "None here either" }));
    expect(await notifications().findByText("No code here")).toBeInTheDocument();
  });
});
