import { describe, expect, it } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CodeBlock } from "./code-block";

const long = Array.from({ length: 30 }, (_, i) => `const line${i} = ${i};`).join("\n");

describe("CodeBlock", () => {
  it("highlights code once the highlighter loads", async () => {
    const { container } = render(<CodeBlock code={'const key = "sk-kaeso";\n'} title="client.ts" lineNumbers highlightLines={[1]} />);
    expect(screen.getByText("client.ts")).toBeInTheDocument();
    expect(container.querySelector(".bg-fill-2")).not.toBeNull();
    await waitFor(() => expect(container.querySelector("code span[style]")).not.toBeNull());
  });

  it("switches between tabs", async () => {
    const { container } = render(
      <CodeBlock
        tabs={[
          { label: "Python", code: "print('hi')", lang: "python" },
          { label: "curl", code: "curl https://kaeso.app", lang: "bash" },
        ]}
      />,
    );
    await userEvent.click(screen.getByRole("tab", { name: "curl" }));
    await waitFor(() => expect(container.querySelector("code")).toHaveTextContent("curl https://kaeso.app"));
  });

  it("collapses long code and expands on request", async () => {
    render(<CodeBlock code={long} lang="text" wrap surface="plain" copy={false} />);
    expect(screen.queryByText("const line29 = 29;")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Show all 30 lines" }));
    expect(screen.getByText("const line29 = 29;")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Show less" }));
    expect(screen.queryByText("const line29 = 29;")).toBeNull();
  });

  it("keeps empty lines and shows everything when collapsing is off", () => {
    render(<CodeBlock code={"a\n\nb"} lang="text" collapseAfter={0} />);
    expect(screen.getByText("a")).toBeInTheDocument();
    expect(screen.getByText("b")).toBeInTheDocument();
  });
});
