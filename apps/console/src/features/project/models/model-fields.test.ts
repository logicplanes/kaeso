import { setFormatLocale } from "@logicplanes/kaeso-ui/format";
import { afterEach, describe, expect, it } from "vitest";
import { contextLabel, decisionLabels, dialectPill, tierLabels, unitPriceLabels } from "./model-fields";
import { changedFields, decisionsOf, draftFrom, draftProblem } from "./model-form";

const limits = [8192, 128_000, 200_000, 1_048_576, 2_000_000];

describe("contextLabel", () => {
  afterEach(() => setFormatLocale("en"));

  it("names a context window as models are named", () => {
    expect(limits.map(contextLabel)).toEqual(["8K", "128K", "200K", "1M", "2M"]);
    expect(contextLabel(131_072)).toBe("128K");
  });

  it("keeps the same short form in German and Chinese", () => {
    setFormatLocale("de");
    expect(limits.map(contextLabel)).toEqual(["8K", "128K", "200K", "1M", "2M"]);
    expect(contextLabel(1_536)).toBe("1,5K");
    setFormatLocale("zh-CN");
    expect(limits.map(contextLabel)).toEqual(["8K", "128K", "200K", "1M", "2M"]);
  });

  it("shows a dash when the limit is unknown", () => {
    expect(contextLabel(null)).toBe("—");
    expect(contextLabel(undefined)).toBe("—");
    expect(contextLabel(0)).toBe("—");
  });
});

describe("tierLabels", () => {
  afterEach(() => setFormatLocale("en"));

  it("names each tier by the prompt length it starts above, with its input and output prices", () => {
    const f = {
      input_price: "10",
      output_price: "50",
      price_tiers: [
        { context_above: 272_000, input_price: "20", output_price: "75" },
        { context_above: 1_048_576, output_price: "0.625" },
      ],
    };
    expect(tierLabels(f)).toEqual(["Above 272K: $20.00 / $75.00", "Above 1M: $10.00 / $0.625"]);
    expect(tierLabels({ price_tiers: [{ context_above: 32_000 }] })).toEqual(["Above 32K: — / —"]);
    expect(tierLabels({})).toEqual([]);
  });
});

describe("unitPriceLabels", () => {
  it("names each unit price with its unit, side and conditions, and says when it is an estimate", () => {
    const f = {
      unit_prices: [
        { per: "image", of: "output", usd: "0.04", quality: "low", resolution: "1K" },
        { per: "image", of: "input", usd: "0.01" },
        { per: "megapixel", of: "output", usd: "0.08", estimate: true },
        { per: "second", of: "output", usd: "0.5" },
      ],
    };
    expect(unitPriceLabels(f)).toEqual([
      "$0.04 per image of output (low, 1K)",
      "$0.01 per image of input",
      "$0.08 per megapixel of output, estimated",
      "$0.50 per second of output",
    ]);
    expect(unitPriceLabels({})).toEqual([]);
    expect(dialectPill("openai_images")).toBe("images");
    expect(dialectPill("minimax_images")).toBe("images");
  });

  it("names audio units, the sound condition and a minimum", () => {
    const f = {
      unit_prices: [
        { per: "million_characters", of: "input", usd: "15" },
        { per: "hour", of: "input", usd: "0.111", minimum_seconds: 10 },
        { per: "minute", of: "output", usd: "0.12" },
        { per: "second", of: "output", usd: "0.05", resolution: "720p", audio: true },
        { per: "second", of: "output", usd: "0.025", audio: false },
        { per: "frame", of: "output", usd: "1" },
      ],
    };
    expect(unitPriceLabels(f)).toEqual([
      "$15.00 per million characters of input",
      "$0.11 per hour of input (at least 10 s)",
      "$0.12 per minute of output",
      "$0.05 per second of output (720p, with sound)",
      "$0.03 per second of output (without sound)",
      "$1.00 per frame of output",
    ]);
    expect(dialectPill("openai_speech")).toBe("speech");
    expect(dialectPill("openai_transcriptions")).toBe("transcriptions");
    expect(dialectPill("elevenlabs")).toBe("elevenlabs");
  });

  it("names prices per video by their length", () => {
    const f = {
      unit_prices: [
        { per: "video", of: "output", usd: "0.28", resolution: "768p", duration: 6 },
        { per: "video", of: "output", usd: "0.2" },
      ],
    };
    expect(unitPriceLabels(f)).toEqual(["$0.28 per video of output (768p, 6 s long)", "$0.20 per video of output"]);
    expect(dialectPill("xai_videos")).toBe("videos");
    expect(dialectPill("veo")).toBe("videos");
  });
});

describe("decisions", () => {
  it("lists what a decision model answers and its limits", () => {
    expect(decisionLabels({ only: true, max_questions: 1000, max_options: 255 })).toEqual([
      "Decisions only",
      "Yes/no, Choice, Score",
      "At most 1,000 questions",
      "At most 255 options per choice",
    ]);
    expect(decisionLabels({ question_types: ["choice", "box"], max_levels: 10, max_images: 4 })).toEqual([
      "Decisions and generation",
      "Choice, Box",
      "At most 10 levels per score",
      "At most 4 images",
    ]);
    expect(decisionLabels({ question_types: ["extract"] })[1]).toBe("extract");
    expect(dialectPill("openai_chat", true)).toBe("decisions");
    expect(dialectPill("system_one")).toBe("decisions");
    expect(dialectPill("openai_chat")).toBe("chat");
  });

  it("edits decisions as an output and changes nothing a model already had", () => {
    const luna = { output_modalities: ["text"], decisions: { max_questions: 400, only: false } };
    const draft = draftFrom(luna);
    expect(draft.output_modalities).toEqual(["text", "decisions"]);
    expect(changedFields(draft, luna, [])).toEqual({});
    expect(changedFields({ ...draft, output_modalities: ["decisions"] }, luna, [])).toEqual({
      output_modalities: [],
      decisions: { only: true, max_questions: 400 },
    });
    expect(changedFields({ ...draft, output_modalities: ["text"] }, luna, [])).toEqual({ decisions: null });
    expect(decisionsOf({ max_images: 2, only: false, question_types: ["choice"] })).toEqual({ question_types: ["choice"], max_images: 2 });
    expect(decisionsOf(null)).toBeNull();
  });

  it("asks a custom decision model for its inputs and input price only", () => {
    const custom = { ...draftFrom({}), output_modalities: ["decisions"] };
    expect(draftProblem(custom, true)).toBe("A custom model needs its inputs and outputs.");
    expect(draftProblem({ ...custom, input_modalities: ["text"] }, true)).toBe("A custom decision model needs its input price.");
    expect(draftProblem({ ...custom, input_modalities: ["text"], input_price: "0.042" }, true)).toBeNull();
  });
});
