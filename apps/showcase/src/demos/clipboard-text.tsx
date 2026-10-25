import { ClipboardText, CopyButton, InlineCopyText, Inline } from "@logicplanes/kaeso-ui";
import { StateSection, Matrix } from "~/lib/states";

export const meta = {
  title: "ClipboardText",
  description: "Copy a value: a read-only pill field, a copy circle, or inline text that copies itself.",
  group: "Components",
  entry: "copy-button",
  import: "ClipboardText, CopyButton, InlineCopyText",
  keyboard: [
    ["Enter", "Copies"],
    ["Space", "Copies"],
  ],
  a11y: [
    "The copy circle's name changes to “Copied” for 1.2 seconds and the tooltip says it.",
    "InlineCopyText announces “Copied” through a polite live region.",
    "Nothing changes size when the state changes.",
  ],
};
export const examples = [
  { id: "field", title: "ClipboardText", fn: "Field", description: "A read-only pill with the value in mono and a copy circle inside." },
  { id: "button", title: "CopyButton", fn: "Button" },
  { id: "inline", title: "InlineCopyText", fn: "InlineText" },
];
export function Field() {
  return (
    <div className="w-400">
      <ClipboardText value="https://kaeso.example.com/v1" label="Copy base URL" />
    </div>
  );
}
export function Button() {
  return (
    <Inline gap={12}>
      <CopyButton value="sk-kso-AbC123…wxyz" label="Copy redacted key" />
      <CopyButton value="req_01JAR4X2" label="Copy id" variant="secondary" />
    </Inline>
  );
}
export function InlineText() {
  return (
    <p className="text-body text-fg-secondary">
      Request <InlineCopyText value="req_01JAR4X2">req_01JAR4X2</InlineCopyText> took 1.2 s.
    </p>
  );
}
export function States() {
  return (
    <>
      <StateSection title="ClipboardText sizes">
        <div className="flex w-400 flex-col gap-12">
          {(["sm", "md", "lg"] as const).map((s) => (
            <ClipboardText key={s} size={s} value="https://kaeso.example.com/v1" />
          ))}
          <ClipboardText value="https://kaeso.example.com/v1/a-very-long-path/that-overflows/and-truncates/with-an-ellipsis" />
        </div>
      </StateSection>
      <Matrix
        title="CopyButton"
        rows={[{ label: "ghost" }, { label: "secondary" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
          { label: "Pressed", props: { "data-force": "pressed" } },
        ]}
        render={(r, p) => <CopyButton value="x" variant={r.label as "ghost"} {...p} />}
      />
      <Matrix
        title="InlineCopyText"
        rows={[{ label: "inline" }]}
        columns={[
          { label: "Rest", props: {} },
          { label: "Hover", props: { "data-force": "hover" } },
          { label: "Focus", props: { "data-force": "focus" } },
        ]}
        render={(_, p) => <InlineCopyText value="key_01JA1" {...p} />}
      />
    </>
  );
}
