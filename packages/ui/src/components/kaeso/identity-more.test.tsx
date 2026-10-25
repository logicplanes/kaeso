import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { KeyDisplay, ModelLabel, ProviderBadge, ProviderCard, StatusCode, KAESO_IDENTITY_VARIANTS, providerName } from "./identity.js";
import { providerMark } from "@logicplanes/kaeso-tokens";
import { PROVIDER_MARKS } from "./provider-marks.js";
import { Slider } from "../slider/slider.js";
import { LocaleProvider } from "../../i18n/index.js";
import { TooltipProvider } from "../tooltip/tooltip-provider.js";
import { hoverWithMouse } from "../../../test/hover.js";
import { lazyModulesSettled } from "../../utils/lazy-module.js";

afterEach(() => vi.useRealTimers());

describe("identity parts", () => {
  it("draws provider marks, names from the caller, and the name's first letter for providers without a mark", () => {
    render(
      <>
        <ProviderBadge provider="openai" name="OpenAI" withName />
        <ProviderBadge provider="acme" name="Zenith Labs" size="xl" />
        <ProviderBadge provider="302ai" name=" 302.AI " />
      </>,
    );
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Zenith Labs" })).toHaveTextContent(/^Z$/);
    expect(screen.getByRole("img", { name: "302.AI" })).toHaveTextContent(/^3$/);
  });

  it("falls back to the provider id without a name", () => {
    render(
      <>
        <ProviderBadge provider="gemini" withName />
        <ProviderBadge provider="cohere" name="" />
        <ModelLabel provider="vertex" model="gemini-3-pro" />
      </>,
    );
    expect(screen.getByText("gemini")).toBeVisible();
    expect(screen.getByRole("img", { name: "cohere" })).toHaveTextContent(/^C$/);
    expect(screen.getByRole("img", { name: "vertex" })).toHaveTextContent(/^V$/);
    expect(providerName("gemini")).toBe("gemini");
    expect(providerName("gemini", "  ")).toBe("gemini");
    expect(providerName("gemini", "Gemini")).toBe("Gemini");
  });

  it("names the provider of a model label for screen readers", () => {
    render(<ModelLabel provider="anthropic" providerName="Anthropic" model="claude-sonnet-4-5" name="Claude Sonnet 4.5" />);
    expect(screen.getByRole("img", { name: "Anthropic" })).toBeInTheDocument();
  });

  it("names a model's capabilities", () => {
    render(<ModelLabel provider="anthropic" model="claude-sonnet-4-5" name="Claude Sonnet 4.5" capabilities={["tools", "vision", "structuredOutput"]} />);
    const img = screen.getByRole("img", { name: "Capabilities: Tool calling, Image input, Structured output" });
    expect(img.querySelectorAll("a[href], button, input, select, textarea, [tabindex]")).toHaveLength(0);
  });

  it("keeps a model's capabilities out of the tab order and still names each one on hover after the delay", async () => {
    render(
      <TooltipProvider>
        <ModelLabel provider="anthropic" model="claude-sonnet-4-5" capabilities={["tools", "reasoning"]} />
        <button type="button">After</button>
      </TooltipProvider>,
    );
    await userEvent.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    const img = screen.getByRole("img", { name: "Capabilities: Tool calling, Reasoning" });
    await act(() => lazyModulesSettled());
    const tip = () => [...document.querySelectorAll("[data-side]")].find((el) => el.textContent === "Tool calling");
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    hoverWithMouse(img.firstElementChild!);
    await act(() => vi.advanceTimersByTimeAsync(499));
    expect(tip()).toBeUndefined();
    await act(() => vi.advanceTimersByTimeAsync(1));
    expect(tip()).toHaveAttribute("data-open");
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
    render(<KeyDisplay redacted="sk-kso-••••1a2b" state="expired" access="both" copy={false} />);
    expect(screen.getByText("Expired")).toBeInTheDocument();
    expect(screen.getByText("Inference + platform")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("shows a provider card with its key and actions", () => {
    render(
      <ProviderCard provider="openai" name="OpenAI" connected status="Healthy" keyLabel="sk-…9f" detail="Added by Ada" actions={<button>Rotate</button>} />,
    );
    const card = screen.getByRole("region", { name: "OpenAI" });
    expect(card).toHaveTextContent("Connected");
    expect(card).toHaveTextContent("Added by Ada");
    render(<ProviderCard provider="anthropic" name="Anthropic" connected={false} status="Add a key to use Anthropic" />);
    expect(screen.getByRole("region", { name: "Anthropic" })).toHaveTextContent("Not connected");
    render(<ProviderCard provider="acme" connected={false} status="No key yet" />);
    expect(screen.getByRole("heading", { name: "acme" })).toBeInTheDocument();
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

const MARKED_PROVIDERS = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  deepseek: "DeepSeek",
  xai: "SpaceX",
  togetherai: "Together AI",
  groq: "Groq",
  openrouter: "OpenRouter",
  mistral: "Mistral",
};
const PARAMS: Record<string, number> = { m: 2, l: 2, h: 1, v: 1, c: 6, s: 4, q: 4, t: 2, a: 7, z: 0 };

function pathEndpoints(d: string) {
  const NUMBER = /^[\s,]*(-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?)/;
  const FLAG = /^[\s,]*([01])/;
  let rest = d;
  const take = (pattern: RegExp) => {
    const match = pattern.exec(rest);
    expect(match, `a number at "${rest.slice(0, 12)}"`).not.toBeNull();
    rest = rest.slice(match![0].length);
    return Number(match![1]);
  };
  const points: [number, number][] = [];
  let x = 0;
  let y = 0;
  let startX = 0;
  let startY = 0;
  let command = "";
  while ((rest = rest.trimStart())) {
    if (/^[a-zA-Z]/.test(rest)) {
      command = rest[0]!;
      rest = rest.slice(1);
    }
    const lower = command.toLowerCase();
    expect(Object.keys(PARAMS)).toContain(lower);
    const relative = command === lower;
    if (lower === "z") {
      x = startX;
      y = startY;
      continue;
    }
    const args = Array.from({ length: PARAMS[lower]! }, (_, n) => take(lower === "a" && (n === 3 || n === 4) ? FLAG : NUMBER));
    if (lower === "h") x = relative ? x + args[0]! : args[0]!;
    else if (lower === "v") y = relative ? y + args[0]! : args[0]!;
    else {
      const [ex, ey] = args.slice(-2) as [number, number];
      x = relative ? x + ex : ex;
      y = relative ? y + ey : ey;
    }
    if (lower === "m") {
      startX = x;
      startY = y;
      command = relative ? "l" : "L";
    }
    points.push([x, y]);
  }
  return points;
}

describe("provider marks", () => {
  it.each(Object.entries(MARKED_PROVIDERS))("draws %s's own mark, named %s", (id, name) => {
    render(<ProviderBadge provider={id} name={name} />);
    const badge = screen.getByRole("img", { name });
    const svg = badge.querySelector("svg");
    expect(badge).toHaveTextContent(/^$/);
    expect(badge).toHaveClass(PROVIDER_MARKS[id]!.color);
    expect(svg).toHaveAttribute("viewBox", "0 0 24 24");
    expect(svg).toHaveAttribute("fill", "currentColor");
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).toHaveAttribute("focusable", "false");
    const paths = svg!.querySelectorAll("path");
    expect(paths).toHaveLength(1);
    expect(paths[0]).toHaveAttribute("d", PROVIDER_MARKS[id]!.d);
    expect(paths[0]!.getAttributeNames()).toEqual(["d"]);
  });

  it("has a mark for every built-in provider and for nothing else", () => {
    expect(Object.keys(PROVIDER_MARKS).sort()).toEqual(Object.keys(MARKED_PROVIDERS).sort());
  });

  it("draws each mark in its owner's one-colour version for each theme, or in the text colour where no colour is set", () => {
    for (const [id, mark] of Object.entries(PROVIDER_MARKS)) {
      if (mark.color === "text-fg") expect(providerMark, id).not.toHaveProperty(id);
      else {
        expect(mark.color, id).toBe(`text-mark-${id}`);
        expect(providerMark, id).toHaveProperty(id);
      }
    }
    expect(providerMark.openrouter).toEqual({ light: "#03080a", dark: "#fcfcfe" });
  });

  it.each(Object.entries(PROVIDER_MARKS))("keeps %s's mark on the 24px grid with at most three decimals", (_, { d }) => {
    expect(d).toMatch(/^M/);
    expect(d).not.toMatch(/\d\.\d{4}/);
    const points = pathEndpoints(d);
    expect(points.length).toBeGreaterThan(2);
    const xs = points.map((p) => p[0]);
    const ys = points.map((p) => p[1]);
    for (const v of [...xs, ...ys]) {
      expect(v).toBeGreaterThanOrEqual(-0.25);
      expect(v).toBeLessThanOrEqual(24.25);
    }
    expect(Math.max(...xs) - Math.min(...xs)).toBeGreaterThan(12);
  });

  it.each(Object.keys(KAESO_IDENTITY_VARIANTS.provider))("draws every mark at the %s size", (size) => {
    render(
      <>
        {Object.entries(MARKED_PROVIDERS).map(([id, name]) => (
          <ProviderBadge key={id} provider={id} name={name} size={size as keyof typeof KAESO_IDENTITY_VARIANTS.provider} />
        ))}
      </>,
    );
    for (const badge of screen.getAllByRole("img")) expect(badge.querySelector("svg path")).not.toBeNull();
  });

  it("writes the name next to the mark and hides the mark from screen readers", () => {
    const { container } = render(<ProviderBadge provider="togetherai" name="Together AI" withName size="sm" />);
    expect(screen.getByText("Together AI")).toBeVisible();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(container.querySelector("[aria-hidden='true'] svg path")).not.toBeNull();
  });
});
