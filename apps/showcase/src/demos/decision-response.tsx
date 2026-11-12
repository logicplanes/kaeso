import { DecisionResponse, ResponseMetrics, type DecisionQuestion } from "@logicplanes/kaeso-ui/playground";
import { triageAnswers, triageQuestions } from "~/fixtures/playground";
import { StateSection } from "~/lib/states";

export const meta = {
  title: "DecisionResponse",
  description:
    "A decision model's answers, one row per question: the answer, the confidence and a probability bar for every option or level; a box drawn on its image.",
  group: "Playground",
  entry: "playground",
  import: "DecisionResponse",
  barrel: false,
  a11y: [
    "The response is an article named after its model and busy while the answers are on their way; screen readers hear when they arrive.",
    "Each question is a list item named after the question; every probability is a meter whose value is read as text.",
    "A box's edges are numbers under the image as well as a line on it, so the answer doesn't depend on seeing the picture.",
    "Show all is a real button that says how many options it reveals and whether they are shown.",
  ],
};
export const examples = [
  { id: "answers", title: "Answers", fn: "Answers", minHeight: 640 },
  { id: "waiting", title: "Waiting", description: "Every row holds its room until the answers arrive.", fn: "Waiting", minHeight: 360 },
];

const receipt = { src: "/demos/receipt.svg", alt: "A receipt from Northwind Labs" };
const metrics = <ResponseMetrics duration={640} inputTokens={1312} outputTokens={0} cost="0.000394" />;
const textQuestions = triageQuestions.slice(0, 3);
const regions = Array.from({ length: 14 }, (_, i) => ({
  value: [
    "eu-west",
    "eu-central",
    "us-east",
    "us-west",
    "ap-south",
    "ap-east",
    "sa-east",
    "af-south",
    "me-central",
    "ca-central",
    "eu-north",
    "ap-southeast",
    "us-central",
    "eu-south",
  ][i]!,
}));
const regionQuestion: DecisionQuestion[] = [{ name: "region", kind: "choice", options: regions }];
const regionAnswer = [
  { kind: "choice" as const, option: 2, probabilities: regions.map((_, i) => (i === 2 ? 0.62 : i === 3 ? 0.21 : i === 12 ? 0.11 : 0.004)) },
];

export function Answers() {
  return (
    <div className="w-full max-w-480">
      <DecisionResponse
        questions={triageQuestions}
        answers={triageAnswers}
        image={receipt}
        footer={metrics}
        onRetry={() => undefined}
        aria-label="GPT-6 Luna response"
      />
    </div>
  );
}
export function Waiting() {
  return (
    <div className="w-full max-w-480">
      <DecisionResponse questions={textQuestions} status="pending" footer={<ResponseMetrics live />} aria-label="GPT-6 Luna response" />
    </div>
  );
}
export function States() {
  const none = () => undefined;
  return (
    <div className="flex w-640 flex-col">
      <StateSection title="Waiting">
        <DecisionResponse questions={triageQuestions} status="pending" image={receipt} />
      </StateSection>
      <StateSection title="Answered">
        <DecisionResponse questions={triageQuestions} answers={triageAnswers} image={receipt} footer={metrics} onRetry={none} />
      </StateSection>
      <StateSection title="A box without its image">
        <DecisionResponse questions={triageQuestions.slice(3)} answers={triageAnswers.slice(3)} />
      </StateSection>
      <StateSection title="Refused">
        <DecisionResponse
          questions={textQuestions}
          answers={[triageAnswers[0], { kind: "refusal", reason: "The question asks for personal data about a named person." }, triageAnswers[2]]}
        />
      </StateSection>
      <StateSection title="Missing answers and probabilities">
        <DecisionResponse questions={textQuestions} answers={[undefined, { kind: "choice", option: 2 }, { kind: "score", score: 1.2 }]} />
      </StateSection>
      <StateSection title="A long choice">
        <DecisionResponse questions={regionQuestion} answers={regionAnswer} maxBars={5} />
      </StateSection>
      <StateSection title="Failed">
        <DecisionResponse
          questions={textQuestions}
          status="error"
          error="The provider refused the request as invalid: `questions.team` needs at least two options."
          onRetry={none}
        />
      </StateSection>
    </div>
  );
}
