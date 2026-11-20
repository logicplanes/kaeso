import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Announcement, ModelCard } from "./model-card.js";

describe("ModelCard", () => {
  it("names the model under its mark with its facts and action", () => {
    render(
      <ModelCard
        mark={<span>OpenAI</span>}
        name="GPT-5"
        model="gpt-5"
        badge={<span>New</span>}
        detail="Released 7 Aug 2025"
        facts={[{ label: "Context", value: "400K" }]}
        action={<button type="button">Add to this project</button>}
      />,
    );
    const card = screen.getByRole("heading", { name: "GPT-5", level: 3 }).closest("article") as HTMLElement;
    expect(card).toHaveTextContent(/OpenAI.*New.*GPT-5.*gpt-5.*Released 7 Aug 2025.*Context.*400K/);
    expect(screen.getByRole("button", { name: "Add to this project" })).toBeInTheDocument();
  });

  it("wraps a fact's value instead of cutting it, so a price in two currencies reads whole", () => {
    render(
      <ModelCard
        mark={<span>A</span>}
        name="GPT-6.2 Astra"
        model="gpt-6.2-astra"
        facts={[{ label: "Pro 1 Mio. Token", value: "8,00 $ rein, 40,00 $ raus" }]}
      />,
    );
    const value = screen.getByText("8,00 $ rein, 40,00 $ raus");
    expect(value).not.toHaveClass("truncate");
    expect(value).toHaveClass("wrap-break-word");
  });

  it("leaves out the badge, facts and action it isn't given", () => {
    render(<ModelCard mark={<span>Mistral</span>} name="mistral-large" model="mistral-large" detail="Mistral" facts={[]} />);
    expect(screen.getByRole("heading", { name: "mistral-large" })).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});

describe("Announcement", () => {
  it("leads with its title, the kind of news under it and never above, then the summary, actions and artwork", () => {
    render(
      <Announcement
        eyebrow="New release"
        title="Kaeso 40.5.0"
        description="Search on every list."
        actions={<button type="button">Read what's new</button>}
        art={<svg aria-label="Kaeso 40.5.0 artwork" role="img" />}
      />,
    );
    const section = screen.getByRole("heading", { name: "Kaeso 40.5.0", level: 2 }).closest("section") as HTMLElement;
    expect(section).toHaveTextContent("Kaeso 40.5.0New releaseSearch on every list.Read what's new");
    expect(screen.getByRole("img", { name: "Kaeso 40.5.0 artwork" })).toBeInTheDocument();
  });

  it("shows only its title when that's all it has", () => {
    render(<Announcement title="Kaeso 40.5.0" />);
    expect((screen.getByRole("heading", { name: "Kaeso 40.5.0" }).closest("section") as HTMLElement).textContent).toBe("Kaeso 40.5.0");
  });
});
