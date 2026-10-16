import { beforeAll, describe, expect, it } from "vitest";
import {
  createTailwindMerge,
  getDefaultConfig,
  mergeConfigs,
  validators,
  type Config,
  type DefaultClassGroupIds,
  type DefaultThemeGroupIds,
} from "tailwind-merge";
import { lengths, radius, shadow, type } from "@logicplanes/kaeso-tokens";
import { cn, kaesoMergeConfig } from "./cn.js";
import { kaesoUtilities, sourceClasses } from "../../test/class-names.js";

const colors = [
  "panel",
  "chrome",
  "raised",
  "tooltip",
  "toast",
  "toast-fg",
  "toast-fg-secondary",
  "toast-action",
  "toast-action-hover",
  "toast-danger",
  "fill-1",
  "fill-2",
  "fill-3",
  "item-hover",
  "item-pressed",
  "line-subtle",
  "line",
  "line-strong",
  "panel-ring",
  "fg",
  "fg-secondary",
  "fg-tertiary",
  "fg-disabled",
  "ink",
  "on-ink",
  "focus",
  "overlay",
  "danger-fill",
  "on-danger",
  "success",
  "success-text",
  "success-tint",
  "warning",
  "warning-text",
  "warning-tint",
  "danger",
  "danger-text",
  "danger-tint",
  "info",
  "info-text",
  "info-tint",
  ...[1, 2, 3, 4, 5, 6, 7, 8].flatMap((n) => [`viz-${n}`, `viz-thin-${n}`]),
];
const shadowColors = colors.filter((c) => !(c in shadow));

const fullConfig = (): Config<string, string> =>
  mergeConfigs<DefaultClassGroupIds | "material" | "focus" | "motion", DefaultThemeGroupIds>(getDefaultConfig(), {
    override: { classGroups: { "shadow-color": [{ shadow: [...shadowColors, validators.isArbitraryVariable, validators.isArbitraryValue] }] } },
    extend: {
      theme: {
        color: colors,
        text: Object.keys(type),
        spacing: lengths.map(String),
        radius: Object.keys(radius),
        shadow: Object.keys(shadow),
        "font-weight": ["regular", "medium", "semibold"],
        ease: ["standard", "overlay", "exit", "pulse"],
      },
      classGroups: {
        material: [
          "material-primary",
          "material-secondary",
          "material-ghost",
          "material-soft",
          "material-danger",
          "material-danger-secondary",
          "material-field",
        ],
        focus: ["focus-ring", "focus-ring-inset"],
        motion: ["popup-motion", "dialog-motion", "sheet-motion", "backdrop-motion", "indicator-motion"],
        "min-w": [{ "min-w": ["anchor"] }],
        w: [{ w: ["anchor"] }],
      },
    },
  });
const fullMerge = createTailwindMerge(fullConfig);

const modifiersOf = (token: string) => {
  const modifiers: string[] = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < token.length; i++) {
    const c = token[i];
    if (c === "[" || c === "(") depth++;
    else if (c === "]" || c === ")") depth--;
    else if (c === ":" && depth === 0) {
      modifiers.push(token.slice(start, i));
      start = i + 1;
    }
  }
  return modifiers;
};
const variantsOf = (token: string) =>
  modifiersOf(token)
    .map((m) => `${m}:`)
    .join("");
const restOf = (token: string) => token.slice(variantsOf(token).length);
const isImportant = (rest: string) => rest.startsWith("!") || rest.endsWith("!");
const coreOf = (rest: string) => rest.replace(/^!/, "").replace(/!$/, "");
const importanceTwins = (rest: string) => [coreOf(rest), `${coreOf(rest)}!`, `!${coreOf(rest)}`];
const arbitraryProperty = (core: string) => /^\[([^:\]]+):.*\]$/.exec(core)?.[1];

interface Placement {
  group: string | undefined;
  postfix: boolean;
}

function placer(config: Config<string, string>, arbitraryProperties: string[]) {
  const groups = [...Object.keys(config.classGroups), ...arbitraryProperties.map((p) => `arbitrary..${p}`)];
  const bits = Array.from({ length: groups.length.toString(2).length }, (_, k) => `kaesoprobe-bit-${k}`);
  const postfix = "kaesoprobe-postfix";
  const markers = [...bits, postfix];
  const merge = createTailwindMerge(() => ({
    ...config,
    cacheSize: 0,
    classGroups: { ...config.classGroups, ...Object.fromEntries(markers.map((m) => [m, [m]])) },
    conflictingClassGroups: Object.fromEntries(groups.map((g, i) => [g, bits.filter((_, k) => ((i + 1) >> k) & 1)])),
    conflictingClassGroupModifiers: Object.fromEntries(groups.map((g) => [g, [postfix]])),
  }));
  const marked = { "": markers.join(" "), "!": markers.map((m) => `${m}!`).join(" ") };
  return (rest: string): Placement => {
    const mark = isImportant(rest) ? "!" : "";
    const kept = new Set(merge(`${marked[mark]} ${rest}`).split(" "));
    const code = bits.reduce((n, b, k) => (kept.has(b + mark) ? n : n | (1 << k)), 0);
    return { group: groups[code - 1], postfix: !kept.has(postfix + mark) };
  };
}

const conflictsOf = (config: Config<string, string>, { group, postfix }: Placement) =>
  group === undefined
    ? new Set<string>()
    : new Set([group, ...(config.conflictingClassGroups[group] ?? []), ...(postfix ? (config.conflictingClassGroupModifiers[group] ?? []) : [])]);

describe("cn's Kaeso merger against the full tailwind-merge configuration", () => {
  let utilities: string[] = [];
  let lists: string[] = [];
  beforeAll(async () => {
    const found = sourceClasses();
    utilities = await kaesoUtilities(found.tokens);
    const known = new Set(utilities);
    lists = found.lists.filter((l) => l.split(" ").every((t) => known.has(t)));
  }, 60_000);

  const differences = (inputs: Iterable<string>) => {
    const out: string[] = [];
    for (const input of inputs) {
      const ours = cn(input);
      const full = fullMerge(input);
      if (ours !== full) out.push(`${input} → ${ours} (full: ${full})`);
    }
    return out;
  };

  it("finds the classes the library, the console, the documentation and the showcase use", () => {
    expect(utilities.length).toBeGreaterThan(800);
    expect(utilities).toEqual(expect.arrayContaining(["px-16", "hover:bg-fill-1", "material-primary", "rounded-pill", "not-focus:sr-only", "text-body"]));
    expect(lists.length).toBeGreaterThan(1000);
  });

  it("merges every class list in the sources the same way", () => {
    expect(differences(lists)).toEqual([]);
  });

  it("reads variants and the important mark the same way", () => {
    const parsing = (c: Config<string, string>) => ({ prefix: c.prefix, parse: c.experimentalParseClassName, ordered: [...c.orderSensitiveModifiers].sort() });
    expect(parsing(kaesoMergeConfig())).toEqual(parsing(fullConfig()));
    const variants = [...new Set(["", ...utilities.map(variantsOf)])];
    const unordered = new Map(variants.map((v) => [v, modifiersOf(v).sort().join(" ")]));
    function* across() {
      for (const a of variants) {
        yield* [`${a}px-8 px-16`, `px-8 ${a}px-16`, `${a}px-8 ${a}px-16`, `${a}px-8 ${a}px-16!`, `${a}px-8! ${a}!px-16`];
        for (const b of variants) if (a !== b && unordered.get(a) === unordered.get(b)) yield `${a}px-8 ${b}px-16`;
      }
    }
    expect(differences(across())).toEqual([]);
  });

  it("places every utility they use, with and without the important mark, in groups that conflict with the same groups", () => {
    const rests = [...new Set(utilities.map(restOf).flatMap(importanceTwins))];
    const samples = ["px-16", "text-body/20!", "[mask-type:alpha]", "kaeso"];
    const arbitrary = [...new Set([...rests, ...samples].map(coreOf).flatMap((c) => arbitraryProperty(c) ?? []))];
    const merger = (name: string, config: Config<string, string>, merge: (classes: string) => string) => ({
      name,
      config,
      merge,
      place: placer(config, arbitrary),
    });
    const ours = merger("ours", kaesoMergeConfig(), cn);
    const full = merger("full", fullConfig(), fullMerge);
    expect([ours, full].map((m) => samples.map(m.place))).toEqual(
      [ours, full].map(() => [
        { group: "px", postfix: false },
        { group: "font-size", postfix: true },
        { group: "arbitrary..mask-type", postfix: false },
        { group: undefined, postfix: false },
      ]),
    );

    const misplaced: string[] = [];
    const placeIn = (m: ReturnType<typeof merger>, rest: string) => {
      const place = m.place(rest);
      if (m.merge(`${rest} ${rest}`) !== (place.group ? rest : `${rest} ${rest}`)) misplaced.push(`${rest} (${m.name})`);
      return { key: JSON.stringify(place), group: place.group, conflicts: conflictsOf(m.config, place) };
    };
    type Placed = ReturnType<typeof placeIn>;
    const placed = new Map<string, { rest: string; ours: Placed; full: Placed }>();
    for (const rest of rests) {
      const [inOurs, inFull] = [placeIn(ours, rest), placeIn(full, rest)];
      const key = `${inOurs.key} ${inFull.key}`;
      if (!placed.has(key)) placed.set(key, { rest, ours: inOurs, full: inFull });
    }
    expect(misplaced).toEqual([]);
    expect(placed.size).toBeLessThan(rests.length / 10);

    const drops = (later: Placed, earlier: Placed) => earlier.group !== undefined && later.conflicts.has(earlier.group);
    const disagreements: string[] = [];
    for (const later of placed.values()) {
      for (const earlier of placed.values()) {
        const [a, b] = [drops(later.ours, earlier.ours), drops(later.full, earlier.full)];
        if (a !== b) disagreements.push(`${earlier.rest} ${later.rest} → ${a ? "drops" : "keeps"} ${earlier.rest} (full: ${b ? "drops" : "keeps"})`);
      }
    }
    expect(disagreements).toEqual([]);
  });
});
