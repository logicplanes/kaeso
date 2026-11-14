import { DecisionQuestions, type DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import { triageQuestions } from "~/fixtures/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "DecisionQuestions",
  description:
    "The questions a decision model was asked, read only: each with its type and instructions, then what yes and no, each option or each level means.",
  group: "Playground",
  entry: "playground",
  import: "DecisionQuestions",
  barrel: false,
  a11y: [
    "The questions are an ordered list named Questions; each question is a list item named after the question.",
    "What yes and no, each option or each level means is a description list, so each term is read with its meaning.",
    "A missing description is said in words, never left blank.",
  ],
};
export const examples = [{ id: "questions", title: "Questions", fn: "Questions", minHeight: 560 }];

const bare: DecisionQuestion[] = [
  { name: "is_spam", kind: "predicate", instructions: "Is this message unsolicited advertising?" },
  { name: "language", kind: "choice", options: [{ value: "en" }, { value: "de", description: "German, Swiss German included" }, { value: "zh" }] },
  { kind: "score", instructions: "How formal is the tone?", levels: [{}, { label: "Neutral" }, { label: "Formal" }] },
];

export function Questions() {
  return (
    <div className="w-full max-w-480">
      <DecisionQuestions questions={triageQuestions} />
    </div>
  );
}
export function States() {
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Every kind">
        <DecisionQuestions questions={triageQuestions} />
      </StateSection>
      <StateSection title="Missing names, instructions and descriptions">
        <DecisionQuestions questions={bare} />
      </StateSection>
    </div>
  );
}
