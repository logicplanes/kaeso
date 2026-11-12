import { useState } from "react";
import { QuestionEditor, type DecisionQuestion, type QuestionEditorProps } from "@logicplanes/kaeso-ui/playground";
import { triageQuestions } from "~/fixtures/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "QuestionEditor",
  description:
    "Typed questions for a decision model: yes/no, a choice of options, a score on levels, and a box for models that draw one, each with a name, instructions and what its answers mean.",
  group: "Playground",
  entry: "playground",
  import: "QuestionEditor",
  barrel: false,
  keyboard: [
    ["Tab", "Moves through a question's type, name, instructions, then its options or levels"],
    ["Enter", "In an option or a level, adds the next one below it and moves there"],
  ],
  a11y: [
    "Each question is a group named after its name, or “Question 1” until it has one; every field has a visible label or a name such as “Option 2”.",
    "Remove buttons are named after what they remove, and focus moves to a neighbour after a removal and into every new question, option or level.",
    "At a limit, Add turns off and a line beside it says what the limit is.",
  ],
};
export const examples = [
  { id: "editor", title: "Questions", fn: "Editor", minHeight: 640 },
  { id: "box", title: "A model that draws boxes", description: "Box joins the types.", fn: "Box", minHeight: 360 },
];

export function Editor() {
  const [value, setValue] = useState<DecisionQuestion[]>(triageQuestions.slice(0, 2));
  return (
    <div className="w-full max-w-480">
      <QuestionEditor value={value} onValueChange={setValue} />
    </div>
  );
}
export function Box() {
  const [value, setValue] = useState<DecisionQuestion[]>(triageQuestions.slice(3));
  return (
    <div className="w-full max-w-480">
      <QuestionEditor value={value} onValueChange={setValue} allowBox />
    </div>
  );
}
function Live({ start, ...props }: { start: DecisionQuestion[] } & Partial<QuestionEditorProps>) {
  const [value, setValue] = useState<DecisionQuestion[]>(start);
  return <QuestionEditor {...props} value={value} onValueChange={setValue} />;
}
export function States() {
  const [predicate, choice, score, box] = triageQuestions;
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="No questions">
        <Live start={[]} />
      </StateSection>
      <StateSection title="Yes/no">
        <Live start={[predicate!]} />
      </StateSection>
      <StateSection title="Choice at its limit">
        <Live start={[choice!]} maxOptions={4} />
      </StateSection>
      <StateSection title="Score">
        <Live start={[score!]} />
      </StateSection>
      <StateSection title="Box">
        <Live start={[box!]} allowBox />
      </StateSection>
      <StateSection title="Compact, at the question limit">
        <Live start={[predicate!, { kind: "predicate" }]} layout="compact" maxQuestions={2} />
      </StateSection>
      <StateSection title="Disabled">
        <Live start={[choice!]} disabled />
      </StateSection>
    </div>
  );
}
