import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { figures, tabularText } from "./figures.js";
import { Badge } from "../components/badge/badge.js";
import { List } from "../components/list/list.js";
import { Stat } from "../components/stat/stat.js";
import { ModelCard } from "../components/dashboard/model-card.js";
import { Legend } from "../components/dashboard/legend.js";
import { MetricTabs } from "../components/dashboard/metric-tabs.js";
import { Collapsible } from "../components/collapsible/collapsible.js";
import { Combobox } from "../components/combobox/combobox.js";
import { DecisionResponse } from "../components/playground/decision-response.js";

describe("figures", () => {
  it("sets names and versions with a hyphen between letters and figures in proportional figures", () => {
    for (const text of ["1.3.0 to 1.4.0-rc.2", "40.4.3-rc.2 available", "gpt-5-mini", "checkout-web · Developer", "Pre-release", "o3-mini"]) {
      expect(tabularText(text), text).toBe(false);
      expect(figures(text), text).toBeUndefined();
    }
  });

  it("keeps tabular figures for numbers, dates and times, so they line up", () => {
    for (const text of ["12", "1,204 tokens", "2026-10-13", "13 Oct", "Tue, Oct 13, 02:00 PM", "10月13日 14:00", "−5", "1.3.0 to 1.4.0"]) {
      expect(figures(text), text).toBe("tabular");
    }
    expect(figures(["1.3.0", " to ", 7])).toBe("tabular");
    expect(figures(["1.3.0 to ", "1.4.0-rc.2"])).toBeUndefined();
  });

  it("leaves an element tabular, as it was: its text can't be read before it renders", () => {
    expect(figures(<span>gpt-5-mini</span>)).toBe("tabular");
  });
});

describe("components that take any text", () => {
  it("Badge: a version or a hyphenated word reads as written, a count keeps tabular figures", () => {
    render(
      <>
        <Badge>40.4.3-rc.1 available</Badge>
        <Badge>Pre-release</Badge>
        <Badge>{"+3 projects"}</Badge>
      </>,
    );
    expect(screen.getByText("40.4.3-rc.1 available")).not.toHaveClass("tabular");
    expect(screen.getByText("Pre-release")).not.toHaveClass("tabular");
    expect(screen.getByText("+3 projects")).toHaveClass("tabular");
  });

  it("List: a row's date or note with a version in it, beside dates that keep tabular figures", () => {
    render(
      <List
        aria-label="Updates"
        items={[
          { id: "a", title: "First", date: "Updated to 40.4.3-rc.1" },
          { id: "b", title: "Second", date: "13 Oct" },
        ]}
      />,
    );
    expect(screen.getByText("Updated to 40.4.3-rc.1").parentElement).not.toHaveClass("tabular");
    expect(screen.getByText("13 Oct").parentElement).toHaveClass("tabular");
  });

  it("Stat, ModelCard, Legend and MetricTabs: a model's name as a value", () => {
    render(
      <>
        <Stat value="gpt-5-mini" />
        <Stat value="1,204" />
        <ModelCard
          mark={null}
          name="GPT-5"
          model="gpt-5"
          facts={[
            { label: "Replaces", value: "gpt-4.1-mini" },
            { label: "Context", value: "400k" },
          ]}
        />
        <Legend items={[{ id: "a", label: "Busiest", value: "claude-haiku-4-5", color: "var(--k-viz-1)" }]} />
        <MetricTabs aria-label="Metric" items={[{ id: "m", label: "Top model", value: "o3-mini" }]} value="m" onValueChange={() => {}} />
      </>,
    );
    expect(screen.getByText("gpt-5-mini")).not.toHaveClass("tabular");
    expect(screen.getByText("1,204")).toHaveClass("tabular");
    expect(screen.getByText("gpt-4.1-mini")).not.toHaveClass("tabular");
    expect(screen.getByText("400k")).toHaveClass("tabular");
    expect(screen.getByText("claude-haiku-4-5")).not.toHaveClass("tabular");
    expect(screen.getByText("o3-mini")).not.toHaveClass("tabular");
  });

  it("Collapsible: a hint naming a version", () => {
    render(
      <Collapsible title="Changes" hint="Since 40.4.3-rc.1">
        Body
      </Collapsible>,
    );
    expect(screen.getByText("Since 40.4.3-rc.1")).not.toHaveClass("tabular");
  });

  it("Combobox: an option's note naming a model", async () => {
    const user = userEvent.setup();
    render(
      <Combobox
        aria-label="Model"
        options={[
          { value: "a", label: "Smart", meta: "gpt-5-mini" },
          { value: "b", label: "Cheap", meta: "12" },
        ]}
      />,
    );
    await user.click(screen.getByRole("combobox", { name: "Model" }));
    expect(await screen.findByText("gpt-5-mini")).not.toHaveClass("tabular");
    expect(screen.getByText("12")).toHaveClass("tabular");
  });

  it("DecisionResponse: a score nearest a level named with a hyphen", () => {
    render(
      <DecisionResponse
        questions={[{ name: "fit", kind: "score", levels: [{ label: "Low" }, { label: "Well-known" }] }]}
        answers={[{ kind: "score", score: 0.9 }]}
      />,
    );
    expect(screen.getByText(/nearest Well-known/)).not.toHaveClass("tabular");
  });
});
