import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { TextProvider, translator, useSiteLocale, useText } from "./text";

function Probe() {
  const t = useText();
  return (
    <p>
      {useSiteLocale()}: {t("page.edit")}
    </p>
  );
}

describe("TextProvider", () => {
  it("speaks English without a provider", () => {
    render(<Probe />);
    expect(screen.getByText("en: Edit on GitHub")).toBeInTheDocument();
  });

  it("gives everything under it the language's text", () => {
    render(
      <TextProvider locale="zh-CN">
        <Probe />
      </TextProvider>,
    );
    expect(screen.getByText("zh-CN: 在 GitHub 上编辑")).toBeInTheDocument();
    expect(translator("de")("page.next")).toBe("Weiter");
  });
});
