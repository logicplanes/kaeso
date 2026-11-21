import { StrictMode, useState } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { KindSwitch } from "./tool-calls";

// Each kind is a page of its own: the switch is drawn anew whenever the kind changes.
function Pages() {
  const [kind, setKind] = useState<"models" | "tools">("models");
  return (
    <>
      <KindSwitch key={kind} value={kind} onChange={(next) => setKind(next ?? "models")} />
      <button type="button" onClick={() => setKind(kind === "tools" ? "models" : "tools")}>
        Other way
      </button>
    </>
  );
}

describe("KindSwitch", () => {
  it("gives the focus back to the switch after the arrow keys changed the page, in strict mode too", async () => {
    const user = userEvent.setup();
    render(
      <StrictMode>
        <Pages />
      </StrictMode>,
    );
    screen.getByRole("radio", { name: "Models" }).focus();
    await user.keyboard("{ArrowRight}");
    await waitFor(() => expect(screen.getByRole("radio", { name: "Tools" })).toHaveFocus());
    expect(screen.getByRole("radio", { name: "Tools" })).toBeChecked();
    await user.keyboard("{ArrowLeft}");
    await waitFor(() => expect(screen.getByRole("radio", { name: "Models" })).toHaveFocus());
  });

  it("leaves the focus where it is when the page changed some other way", async () => {
    const user = userEvent.setup();
    render(<Pages />);
    await user.click(screen.getByRole("button", { name: "Other way" }));
    await new Promise((r) => requestAnimationFrame(r));
    expect(screen.getByRole("radio", { name: "Tools" })).toBeChecked();
    expect(screen.getByRole("button", { name: "Other way" })).toHaveFocus();
  });
});
