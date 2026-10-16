import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { KeyDisplay, ModelLabel, ProviderBadge, ProviderCard, StatusCode } from "./identity";
import { Slider } from "../slider/slider";
import { LocaleProvider } from "../../i18n";

describe("identity parts", () => {
  it("draws provider marks, letters for unknown providers, and names", () => {
    render(
      <>
        <ProviderBadge provider="openai" withName />
        <ProviderBadge provider="acme" size="xl" />
      </>,
    );
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
    expect(screen.getByText("A")).toBeInTheDocument();
  });

  it("names a model's capabilities", () => {
    render(<ModelLabel provider="anthropic" model="claude-sonnet-4-5" name="Claude Sonnet 4.5" capabilities={["tools", "vision", "structuredOutput"]} />);
    expect(screen.getByRole("img", { name: "Capabilities: Tool calling, Image input, Structured output" })).toBeInTheDocument();
  });

  it("writes status codes with their text, and unknown codes plainly", () => {
    render(
      <>
        <StatusCode code={429} showText />
        <StatusCode code={503} />
        <StatusCode code={299} />
      </>,
    );
    expect(screen.getByText("429").nextSibling).toHaveTextContent(/./);
    expect(screen.getByText("503")).toHaveClass("text-danger-text");
    expect(screen.getByText("299")).toHaveClass("text-fg");
  });

  it("shows a key with its state and access, without copy", () => {
    render(<KeyDisplay redacted="sk-kaeso-••••1a2b" state="expired" access="both" copy={false} />);
    expect(screen.getByText("Expired")).toBeInTheDocument();
    expect(screen.getByText("Inference + platform")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows a provider card with its key and actions", () => {
    render(<ProviderCard provider="openai" connected status="Healthy" keyLabel="sk-…9f" detail="Added by Ada" actions={<button>Rotate</button>} />);
    const card = screen.getByRole("region", { name: "OpenAI" });
    expect(card).toHaveTextContent("Connected");
    expect(card).toHaveTextContent("Added by Ada");
    render(<ProviderCard provider="anthropic" connected={false} status="Add a key to use Anthropic" />);
    expect(screen.getByRole("region", { name: "Anthropic" })).toHaveTextContent("Not connected");
  });
});

describe("Slider", () => {
  it("reports changes and commits, with ticks", () => {
    const onChange = vi.fn();
    const onCommit = vi.fn();
    const { container } = render(
      <Slider aria-label="Temperature" defaultValue={1} min={0} max={2} step={0.5} ticks onValueChange={onChange} onValueCommitted={onCommit} name="t" />,
    );
    fireEvent.keyDown(screen.getByRole("slider", { name: "Temperature" }), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith(1.5);
    expect(container.querySelectorAll("[aria-hidden] > span.absolute")).toHaveLength(5);
  });

  it("names both ends of a range and writes German numbers", () => {
    const onChange = vi.fn();
    render(
      <LocaleProvider locale="de" messages={{}}>
        <Slider
          value={[0.2, 0.8]}
          min={0}
          max={1}
          step={0.1}
          ticks={[0, 0.5, 1]}
          getAriaLabel={(i) => (i === 0 ? "From" : "To")}
          onValueChange={onChange}
          data-force="focus dragging"
          disabled
        />
      </LocaleProvider>,
    );
    expect(screen.getByRole("slider", { name: "From" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "To" })).toBeInTheDocument();
    expect(screen.getAllByText("0,2").length).toBeGreaterThan(0);
  });
});
