import { describe, expect, it } from "vitest";
import { render, renderHook, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { LocaleContext } from "../../i18n/context.js";
import de from "../../i18n/locales/de.js";
import zhCN from "../../i18n/locales/zh-CN.js";
import { AGENT_MODELS, AgentAttribution, AgentAvatar, agentModel, modelName, useModelName } from "./index.js";

const glyphOf = (el: HTMLElement) => el.querySelector("svg")?.innerHTML;

function inLocale(locale: string, messages: Record<string, string>) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <LocaleContext.Provider value={{ locale, messages }}>{children}</LocaleContext.Provider>;
  };
}

describe("AgentAvatar", () => {
  it("is a picture named after the agent", () => {
    render(<AgentAvatar model="claude" name="Release helper" />);
    expect(screen.getByRole("img", { name: "Release helper" }).querySelector("svg")).toBeInTheDocument();
  });

  it("draws a different glyph for every model family", () => {
    const glyphs = AGENT_MODELS.map((model) => {
      const { unmount } = render(<AgentAvatar model={model} name={model} />);
      const glyph = glyphOf(screen.getByRole("img", { name: model }));
      unmount();
      return glyph;
    });
    expect(new Set(glyphs).size).toBe(AGENT_MODELS.length);
  });

  it("draws a model family it doesn't know like unknown", () => {
    render(
      <>
        <AgentAvatar model="falcon" name="Nightly fixer" />
        <AgentAvatar model="unknown" name="Docs bot" />
        <AgentAvatar model="Claude" name="Shouting bot" />
        <AgentAvatar model="constructor" name="Prototype bot" />
      </>,
    );
    const unknown = glyphOf(screen.getByRole("img", { name: "Docs bot" }));
    expect(glyphOf(screen.getByRole("img", { name: "Nightly fixer" }))).toBe(unknown);
    expect(glyphOf(screen.getByRole("img", { name: "Shouting bot" }))).toBe(unknown);
    expect(glyphOf(screen.getByRole("img", { name: "Prototype bot" }))).toBe(unknown);
  });

  it("leaves the reading to the text beside it when decorative", () => {
    const { container } = render(<AgentAvatar model="gpt" name="Release helper" size={40} decorative />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });
});

describe("AgentAttribution", () => {
  it("names the person and the agent that acted for them", () => {
    render(<AgentAttribution name="Ada Okafor" agent={{ name: "Release helper", model: "claude" }} />);
    const picture = screen.getByRole("img", { name: "Ada Okafor via Release helper (Claude)" });
    expect(picture).toHaveTextContent("AO");
    expect(picture.querySelector("[data-agent-badge] svg")).toBeInTheDocument();
    expect(screen.getAllByRole("img")).toHaveLength(1);
  });

  it("shows the person's picture under the badge", () => {
    const { container } = render(
      <AgentAttribution name="Ada Okafor" src="https://example.com/ada.png" size={40} agent={{ name: "Nightly fixer", model: "falcon" }} />,
    );
    expect(container.querySelector("img")).toHaveAttribute("src", "https://example.com/ada.png");
    expect(screen.getByRole("img", { name: "Ada Okafor via Nightly fixer (Unknown)" })).toBeInTheDocument();
  });

  it("speaks the page's language", () => {
    render(
      <>
        <AgentAttribution name="Ada Okafor" agent={{ name: "Docs bot", model: "unknown" }} />
        <AgentAttribution name="Lin Park" agent={{ name: "Key rotation", model: "deepseek" }} />
      </>,
      { wrapper: inLocale("de", de) },
    );
    expect(screen.getByRole("img", { name: "Ada Okafor über Docs bot (Unbekannt)" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Lin Park über Key rotation (DeepSeek)" })).toBeInTheDocument();
  });
});

describe("modelName", () => {
  it("names each family as it names itself and anything else Unknown", () => {
    expect(AGENT_MODELS.map((m) => modelName(m))).toEqual([
      "Claude",
      "GPT",
      "Gemini",
      "Gemma",
      "Grok",
      "Llama",
      "Mistral",
      "DeepSeek",
      "Qwen",
      "Kimi",
      "GLM",
      "MiniMax",
      "Command",
      "Nova",
      "Phi",
      "Granite",
      "Jamba",
      "ERNIE",
      "Hunyuan",
      "Doubao",
      "Yi",
      "Nemotron",
      "OLMo",
      "Sonar",
      "Reka",
      "Unknown",
    ]);
    expect(modelName("falcon")).toBe("Unknown");
    expect(agentModel("toString")).toBe("unknown");
    expect(agentModel("qwen")).toBe("qwen");
  });

  it("translates Unknown and keeps the product names", () => {
    const { result } = renderHook(() => useModelName(), { wrapper: inLocale("zh-CN", zhCN) });
    expect(result.current("gemini")).toBe("Gemini");
    expect(result.current("falcon")).toBe("未知");
  });
});
