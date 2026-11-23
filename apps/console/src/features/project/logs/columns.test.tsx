import { render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { DataTable } from "@logicplanes/kaeso-ui";
import { logRow } from "~/test/fixtures/project-logs";
import { externalSource } from "~/test/fixtures/project";
import { apply, load } from "~/i18n";
import { COLUMNS, isEmbedding, isRerank, logColumns, logsLayout, providerInModel, type LogsLayout } from "./columns";

const OPTIONAL = ["source", "request", "agent", "client", "alias", "formats", "cache", "fallbacks", "upstream"];
const shown = COLUMNS.filter((c) => !OPTIONAL.includes(c));

describe("logsLayout", () => {
  it("keeps the provider's name while the columns people compare fit, and its mark alone below that", () => {
    expect(logsLayout(0, false, true, shown)).toBe("wide");
    expect(logsLayout(1200, false, true, shown)).toBe("wide");
    expect(logsLayout(1000, false, true, shown)).toBe("medium");
    expect(logsLayout(1000, false, false, ["time", "status", "model", "provider"])).toBe("wide");
    expect(logsLayout(1000, false, true, ["duration"])).toBe("wide");
    expect(logsLayout(700, false, true, shown)).toBe("narrow");
    expect(logsLayout(1200, true, true, shown)).toBe("phone");
  });

  it("keeps every column at full width in a 1440 pixel window, in every language", async () => {
    const grid1440 = 1136;
    for (const language of ["en", "de", "zh-CN"] as const) {
      apply(await load(language));
      expect(logsLayout(grid1440, false, true, shown), language).toBe("wide");
    }
    apply(await load("en"));
  });
});

function table(layout: LogsLayout, spansDays = true) {
  const all = logColumns({ spansDays, layout, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => ({ openai: "OpenAI" })[id] ?? id });
  render(<DataTable aria-label="Requests" data={[logRow()]} columns={COLUMNS.map((c) => all[c])} getRowId={(r) => r.id} />);
  return screen.getAllByRole("cell");
}

describe("logColumns", () => {
  beforeEach(() => setFormatTimeZone("UTC"));

  it("names the time zone of the start time and shows the provider by name", () => {
    const cells = table("wide");
    expect(screen.getAllByRole("columnheader")[0]).toHaveTextContent("Started (UTC)");
    expect(cells[0]?.textContent).toMatch(/\d{2}:\d{2}:\d{2}$/);
    expect(screen.getByRole("columnheader", { name: /^Provider/ })).toBeInTheDocument();
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
  });

  it("puts the provider's mark before the model id, named by a tooltip, when the table is narrower", () => {
    const all = logColumns({
      spansDays: true,
      layout: "medium",
      withProvider: true,
      keyName: (id) => id,
      partyName: (p) => p.id ?? "",
      providerName: (id) => ({ openai: "OpenAI" })[id] ?? id,
    });
    render(<DataTable aria-label="Requests" data={[logRow()]} columns={[all.time, all.status, all.model, all.duration]} getRowId={(r) => r.id} />);
    expect(screen.queryByRole("columnheader", { name: /^Provider/ })).toBeNull();
    expect(screen.queryByText("OpenAI")).toBeNull();
    const model = screen.getAllByRole("cell")[2] as HTMLElement;
    expect(within(model).getByRole("img", { name: "OpenAI" })).toBeInTheDocument();
    expect([...model.querySelectorAll(".\\@container > *")].map((e) => e.textContent)).toEqual(["", "gpt-5.4-mini"]);
  });

  it("folds the provider into the model only where its column would hold the mark alone", () => {
    expect(providerInModel("wide", shown)).toBe(false);
    expect(providerInModel("medium", shown)).toBe(true);
    expect(providerInModel("narrow", shown)).toBe(true);
    expect(providerInModel("phone", shown)).toBe(false);
    expect(
      providerInModel(
        "medium",
        shown.filter((c) => c !== "provider"),
      ),
    ).toBe(false);
    const withoutModel = shown.filter((c) => c !== "model");
    expect(providerInModel("medium", withoutModel)).toBe(false);
    expect(providerInModel("narrow", withoutModel)).toBe(false);
  });

  it("stacks a day's time into a narrow column on a phone, leaving the model the room", () => {
    const [started] = table("phone");
    expect(screen.getAllByRole("row")[1]?.style.gridTemplateColumns).toMatch(/^88px 80px /);
    expect(started?.querySelectorAll("time > *")).toHaveLength(2);
  });

  it("gives the phone's Started column room for its whole header in Chinese", async () => {
    apply(await load("zh-CN"));
    try {
      table("phone");
      expect(screen.getAllByRole("row")[1]?.style.gridTemplateColumns).toMatch(/^96px 80px /);
    } finally {
      apply(await load("en"));
    }
  });

  it("drops the seconds from a day's time on a tablet", () => {
    const [started] = table("narrow");
    expect(started?.textContent).toMatch(/\d{2}:\d{2}$/);
    expect(started?.textContent).not.toMatch(/:\d{2}:\d{2}/);
  });

  it("keeps the seconds within one hour on a phone", () => {
    expect(table("phone", false)[0]?.textContent).toMatch(/^\d{2}:\d{2}:\d{2}$/);
    expect(screen.getAllByRole("columnheader")[0]).toHaveTextContent(/^Started$/);
  });

  it("puts the model first in its cell, then the outcome, the source and the message", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => ({ openai: "OpenAI" })[id] ?? id });
    render(
      <DataTable
        aria-label="Requests"
        data={[logRow({ status: 499, error_kind: "client_disconnected", error_message: "The app closed the connection.", source: "playground" })]}
        columns={[all.model]}
        getRowId={(r) => r.id}
      />,
    );
    const parts = [...screen.getByRole("cell").querySelectorAll(".\\@container > *")].map((e) => e.textContent);
    expect(parts).toEqual(["gpt-5.4-mini", "Client disconnected", "Playground", "The app closed the connection."]);
  });

  it("shows the client, alias, formats, response cache, fallbacks and the provider's request id", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => id });
    const ids = ["client", "alias", "formats", "cache", "fallbacks", "upstream"] as const;
    const rows = [
      logRow({
        id: "req_1",
        client_ip: "203.0.113.9",
        alias: "fast",
        ingress_dialect: "openai_chat",
        egress_dialect: "anthropic_messages",
        response_cache: "hit",
        fallbacks: [
          {
            catalogue_model_id: "cm_1",
            model: "anthropic/claude-haiku-4-5",
            path: "route",
            status: 529,
            error_kind: "overloaded",
            error_message: "",
            duration_ms: 10,
            skipped: false,
            retried: false,
          },
          // Tried again, so not a fallback.
          {
            catalogue_model_id: "cm_1",
            model: "anthropic/claude-haiku-4-5",
            path: "route",
            status: 529,
            error_kind: "overloaded",
            error_message: "",
            duration_ms: 10,
            skipped: false,
            retried: true,
          },
        ],
        upstream_request_id: "req_upstream_1",
      }),
      logRow({
        id: "req_2",
        alias: null,
        ingress_dialect: "openai_chat",
        egress_dialect: "openai_chat",
        response_cache: null,
        fallbacks: [],
        upstream_request_id: "",
      }),
      logRow({ id: "req_3", response_cache: "miss" }),
      logRow({ id: "req_4", response_cache: "bypass" }),
    ];
    render(<DataTable aria-label="Requests" data={rows} columns={ids.map((c) => all[c])} getRowId={(r) => r.id} />);
    const [first, second] = screen.getAllByRole("row").slice(1);
    const texts = (row: HTMLElement | undefined) =>
      within(row as HTMLElement)
        .getAllByRole("cell")
        .map((c) => c.textContent);
    expect(texts(first)).toEqual(["203.0.113.9", "fast", "OpenAI Chat Completions → Anthropic Messages", "Hit", "1", "req_upstream_1"]);
    expect(texts(second)).toEqual(["203.0.113.7", "", "OpenAI Chat Completions", "", "", ""]);
    expect(
      screen
        .getAllByRole("row")
        .slice(3)
        .map((r) => texts(r)[3]),
    ).toEqual(["Miss", "Bypass"]);
  });

  it("names the model the app asked for, then the outcome, when a request was refused before a model was chosen", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => ({ openai: "OpenAI" })[id] ?? id });
    render(
      <DataTable
        aria-label="Requests"
        data={[logRow({ status: 400, model: "", provider: "", error_kind: "invalid_request" })]}
        columns={[all.model]}
        getRowId={(r) => r.id}
      />,
    );
    const parts = [...screen.getByRole("cell").querySelectorAll(".\\@container > *")].map((e) => e.textContent);
    expect(parts).toEqual(["gpt-5.4-mini", "Invalid request"]);
  });

  it("names the provider under a visible header wherever its column is shown on a phone", () => {
    table("phone");
    expect(screen.getByRole("columnheader", { name: /^Provider/ })).toHaveTextContent("Provider");
    expect(screen.getByText("OpenAI")).toBeInTheDocument();
  });

  it("leaves the provider cell empty when a request was refused before a provider was chosen", () => {
    const all = logColumns({ spansDays: true, layout: "narrow", keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => id });
    render(
      <DataTable
        aria-label="Requests"
        data={[logRow({ status: 400, model: "", provider: "", error_kind: "invalid_request" })]}
        columns={[all.provider]}
        getRowId={(r) => r.id}
      />,
    );
    expect(screen.getByRole("cell").textContent).toBe("");
    expect(within(screen.getByRole("cell")).queryByRole("img")).not.toBeInTheDocument();
  });

  it("gives a successful Playground request's tag room sooner than a failed one's", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => ({ openai: "OpenAI" })[id] ?? id });
    render(
      <DataTable
        aria-label="Requests"
        data={[logRow({ source: "playground" }), logRow({ id: "req_0002", source: "playground", status: 429, error_kind: "rate_limited" })]}
        columns={[all.model]}
        getRowId={(r) => r.id}
      />,
    );
    const [ok, failed] = screen.getAllByText("Playground").map((e) => e.closest("[class*='@cell-']"));
    expect(ok).toHaveClass("@cell-sm:inline-flex");
    expect(failed).toHaveClass("@cell-md:inline-flex");
  });

  it("shows what a media request made in place of tokens it didn't count", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => id });
    const zero = { input_tokens: 0, output_tokens: 0, cache_read_tokens: 0, cache_write_tokens: 0, reasoning_tokens: 0, estimated: false };
    const media = (id: string, unit: string, input: number, output: number) => logRow({ id, usage: zero, media: { unit, input, output, detail: "" } });
    render(
      <DataTable
        aria-label="Requests"
        data={[
          media("req_1", "image", 0, 1),
          media("req_2", "character", 17, 0),
          media("req_3", "millisecond", 4200, 0),
          media("req_4", "video", 1, 6000),
          media("req_5", "image", 2, 0),
          logRow({ id: "req_6", media: { unit: "image", input: 1, output: 1, detail: "" } }),
        ]}
        columns={[all.tokens]}
        getRowId={(r) => r.id}
      />,
    );
    const cells = screen.getAllByRole("cell").map((c) => c.textContent);
    expect(cells.slice(0, 5)).toEqual(["1 image", "17 characters", "4.2 s", "1 video · 6 s", "2 images sent"]);
    expect(cells[5]).toMatch(/^\d[\d,]* \/ \d/);
  });

  it("shows how much of the input was read from the cache, and marks a request sent without a breakpoint", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => id });
    const usage = { input_tokens: 25, output_tokens: 10, cache_read_tokens: 75, cache_write_tokens: 0, reasoning_tokens: 0, estimated: false };
    render(
      <DataTable
        aria-label="Requests"
        data={[
          logRow({ usage }),
          logRow({
            id: "req_0002",
            egress_dialect: "anthropic_messages",
            cache_breakpoints: 0,
            usage: { ...usage, input_tokens: 563_000, cache_read_tokens: 0 },
          }),
          logRow({ id: "req_0003", ingress_dialect: "openai_embeddings", egress_dialect: "openai_embeddings" }),
          logRow({ id: "req_0004", egress_dialect: "anthropic_messages", cache_breakpoints: 0, usage: { ...usage, input_tokens: 10, cache_read_tokens: 90 } }),
          logRow({
            id: "req_0005",
            egress_dialect: "anthropic_messages",
            cache_breakpoints: 0,
            usage: { ...usage, input_tokens: 40, cache_read_tokens: 0, cache_write_tokens: 60 },
          }),
        ]}
        columns={[all.tokens]}
        getRowId={(r) => r.id}
      />,
    );
    const [cached, unmarked, embedding, cachedWithoutMarks, writtenWithoutMarks] = screen.getAllByRole("cell");
    expect(cached).toHaveTextContent("100 / 10· 75%");
    expect(within(cached as HTMLElement).getByText("· 75%")).toHaveClass("text-fg-tertiary");
    expect(unmarked).toHaveTextContent("563,000 / 10· 0%");
    expect(within(unmarked as HTMLElement).getByText("· 0%")).toHaveClass("text-warning-text");
    expect(within(unmarked as HTMLElement).getByRole("img", { name: "No cache breakpoint" })).toBeInTheDocument();
    expect(within(cached as HTMLElement).queryByRole("img")).not.toBeInTheDocument();
    expect(embedding).toHaveTextContent(/^120 \/ 48$/);
    expect(cachedWithoutMarks).toHaveTextContent("100 / 10· 90%");
    expect(within(cachedWithoutMarks as HTMLElement).getByText("· 90%")).toHaveClass("text-fg-tertiary");
    expect(writtenWithoutMarks).toHaveTextContent("100 / 10· 0%");
    expect(within(writtenWithoutMarks as HTMLElement).getByText("· 0%")).toHaveClass("text-fg-tertiary");
  });
});

describe("isRerank", () => {
  it("knows rerank requests by either format, which no provider caches, apart from embeddings", () => {
    expect(isRerank({ ingress_dialect: "cohere_rerank", egress_dialect: "cohere_rerank" })).toBe(true);
    expect(isRerank({ ingress_dialect: "cohere_rerank", egress_dialect: "voyage_rerank" })).toBe(true);
    expect(isRerank({ ingress_dialect: "cohere_rerank", egress_dialect: "jina_rerank" })).toBe(true);
    expect(isRerank({ ingress_dialect: "openai_chat", egress_dialect: "openai_chat" })).toBe(false);
    expect(isEmbedding({ ingress_dialect: "cohere_rerank", egress_dialect: "cohere_rerank" })).toBe(false);
  });
});

describe("observed requests", () => {
  it("are tagged, and name the source that reported them where a key would be", () => {
    const sources = [externalSource()];
    const all = logColumns({
      spansDays: true,
      keyName: (id) => id,
      partyName: (p) => p.id ?? "",
      providerName: (id) => id,
      sourceOf: (id) => sources.find((x) => x.id === id),
    });
    const rows = [
      logRow({ id: "req_seen", source: "observed", source_id: "src_claude", key_id: null }),
      logRow({ id: "req_gone", source: "observed", source_id: "src_gone", key_id: null }),
      logRow({ id: "req_play", source: "playground", key_id: null }),
      logRow({ id: "req_api" }),
    ];
    render(<DataTable aria-label="Requests" data={rows} columns={[all.model, all.key, all.source]} getRowId={(r) => r.id} />);
    const [seen, gone, play, api] = screen.getAllByRole("row").slice(1);
    expect(seen).toHaveTextContent("Observed");
    expect(within(seen as HTMLElement).getAllByText("Claude Code, Ada's laptop")).toHaveLength(2);
    expect(within(gone as HTMLElement).getAllByText("A source")).toHaveLength(2);
    expect(play).toHaveTextContent("Playground");
    expect(api).toHaveTextContent("Gateway");
    expect(api).not.toHaveTextContent("Observed");
  });

  it("show a dash for the key without the project's sources", () => {
    const all = logColumns({ spansDays: true, keyName: (id) => id, partyName: (p) => p.id ?? "", providerName: (id) => id });
    render(<DataTable aria-label="Requests" data={[logRow({ key_id: null })]} columns={[all.key]} getRowId={(r) => r.id} />);
    expect(screen.getAllByRole("cell")[0]).toHaveTextContent("—");
  });
});
