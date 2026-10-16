import { Text, KAESO_TEXT_VARIANTS, type TextVariant, type TextTone } from "@logicplanes/kaeso-ui";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "Text",
  description: "Text in the type scale: one scale, used the same way everywhere.",
  group: "Components",
  entry: "text",
  import: "Text",
  a11y: [
    'Text renders a span by default; pass as="p" for paragraphs.',
    "Truncated text gets a tooltip with the full string and becomes focusable so keyboard users can read it.",
    "Tertiary never carries meaning alone (3.23:1 on white).",
  ],
};
export const examples = [
  { id: "scale", title: "Scale", fn: "Scale" },
  { id: "tones", title: "Tones", fn: "Tones" },
  { id: "mono", title: "Mono and tabular", fn: "Mono" },
  { id: "truncate", title: "Truncation", fn: "Truncate" },
];
export function Scale() {
  return (
    <div className="flex flex-col gap-8">
      {(Object.keys(KAESO_TEXT_VARIANTS.variant) as TextVariant[]).map((v) => (
        <Text key={v} variant={v}>
          {v === "metric" ? "$1,284.20" : `${v}: Route each request to the model you choose`}
        </Text>
      ))}
    </div>
  );
}
export function Tones() {
  return (
    <div className="flex flex-col gap-6">
      {(
        [
          ["default", "Provider key saved"],
          ["secondary", "Last used 2 min ago"],
          ["tertiary", "Optional"],
          ["danger", "Unreachable"],
          ["success", "Connected"],
          ["warning", "Expires in 5 days"],
          ["info", "Syncing models"],
        ] as [TextTone, string][]
      ).map(([t, words]) => (
        <Text key={t} tone={t}>
          {t}: {words}
        </Text>
      ))}
    </div>
  );
}
export function Mono() {
  return (
    <div className="flex flex-col gap-6">
      <Text mono>claude-sonnet-4-5</Text>
      <Text tabular>1,204,880 tokens</Text>
    </div>
  );
}
export function Truncate() {
  return (
    <div className="w-240">
      <Text truncate>Checkout assistant · production traffic from the web storefront</Text>
    </div>
  );
}
export function States() {
  return (
    <>
      <StateSection title="Scale in context">
        <Scale />
      </StateSection>
      <StateSection title="Tones">
        <Tones />
      </StateSection>
      <StateSection title="Truncation">
        <Truncate />
      </StateSection>
    </>
  );
}
