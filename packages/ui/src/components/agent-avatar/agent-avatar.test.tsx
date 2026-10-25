import { describe, expect, it } from "vitest";
import { render, renderHook, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { LocaleContext } from "../../i18n/context.js";
import de from "../../i18n/locales/de.js";
import zhCN from "../../i18n/locales/zh-CN.js";
import { AGENT_HARNESSES, AgentAttribution, AgentAvatar, agentHarness, harnessName, useHarnessName } from "./index.js";

const glyphOf = (el: HTMLElement) => el.querySelector("svg")?.innerHTML;

function inLocale(locale: string, messages: Record<string, string>) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return <LocaleContext.Provider value={{ locale, messages }}>{children}</LocaleContext.Provider>;
  };
}

describe("AgentAvatar", () => {
  it("is a picture named after the agent", () => {
    render(<AgentAvatar harness="claude_code" name="Release helper" />);
    expect(screen.getByRole("img", { name: "Release helper" }).querySelector("svg")).toBeInTheDocument();
  });

  it("draws a different glyph for every harness", () => {
    const glyphs = AGENT_HARNESSES.map((harness) => {
      const { unmount } = render(<AgentAvatar harness={harness} name={harness} />);
      const glyph = glyphOf(screen.getByRole("img", { name: harness }));
      unmount();
      return glyph;
    });
    expect(new Set(glyphs).size).toBe(AGENT_HARNESSES.length);
  });

  it("draws a harness it doesn't know like other", () => {
    render(
      <>
        <AgentAvatar harness="aider" name="Nightly fixer" />
        <AgentAvatar harness="other" name="Docs bot" />
        <AgentAvatar harness="constructor" name="Prototype bot" />
      </>,
    );
    const other = glyphOf(screen.getByRole("img", { name: "Docs bot" }));
    expect(glyphOf(screen.getByRole("img", { name: "Nightly fixer" }))).toBe(other);
    expect(glyphOf(screen.getByRole("img", { name: "Prototype bot" }))).toBe(other);
  });

  it("leaves the reading to the text beside it when decorative", () => {
    const { container } = render(<AgentAvatar harness="codex" name="Release helper" size={40} decorative />);
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(container.firstElementChild).toHaveAttribute("aria-hidden", "true");
  });
});

describe("AgentAttribution", () => {
  it("names the person and the agent that acted for them", () => {
    render(<AgentAttribution name="Ada Okafor" agent={{ name: "Release helper", harness: "claude_code" }} />);
    const picture = screen.getByRole("img", { name: "Ada Okafor via Release helper (Claude Code)" });
    expect(picture).toHaveTextContent("AO");
    expect(picture.querySelector("[data-agent-badge] svg")).toBeInTheDocument();
    expect(screen.getAllByRole("img")).toHaveLength(1);
  });

  it("shows the person's picture under the badge", () => {
    const { container } = render(
      <AgentAttribution name="Ada Okafor" src="https://example.com/ada.png" size={40} agent={{ name: "Nightly fixer", harness: "aider" }} />,
    );
    expect(container.querySelector("img")).toHaveAttribute("src", "https://example.com/ada.png");
    expect(screen.getByRole("img", { name: "Ada Okafor via Nightly fixer (Other)" })).toBeInTheDocument();
  });

  it("speaks the page's language", () => {
    render(<AgentAttribution name="Ada Okafor" agent={{ name: "Docs bot", harness: "other" }} />, { wrapper: inLocale("de", de) });
    expect(screen.getByRole("img", { name: "Ada Okafor über Docs bot (Andere)" })).toBeInTheDocument();
  });
});

describe("harnessName", () => {
  it("names each product as it names itself and anything else Other", () => {
    expect(AGENT_HARNESSES.map((h) => harnessName(h))).toEqual(["Claude Code", "Codex", "Cursor", "Gemini CLI", "OpenCode", "Pi", "Other"]);
    expect(harnessName("aider")).toBe("Other");
    expect(agentHarness("toString")).toBe("other");
  });

  it("translates Other and keeps the product names", () => {
    const { result } = renderHook(() => useHarnessName(), { wrapper: inLocale("zh-CN", zhCN) });
    expect(result.current("gemini_cli")).toBe("Gemini CLI");
    expect(result.current("unknown")).toBe("其他");
  });
});
