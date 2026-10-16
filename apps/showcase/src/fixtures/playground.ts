export interface PlaygroundModel {
  value: string;
  provider: string;
  name: string;
  input: number;
  output: number;
  reasoning: boolean;
  latency: number;
  speed: number;
}

export const playgroundModels: PlaygroundModel[] = [
  { value: "gpt-5", provider: "openai", name: "GPT-5", input: 1.25, output: 10, reasoning: true, latency: 1180, speed: 5 },
  { value: "claude-sonnet-4-5", provider: "anthropic", name: "Claude Sonnet 4.5", input: 3, output: 15, reasoning: true, latency: 820, speed: 7 },
  { value: "gpt-5-mini", provider: "openai", name: "GPT-5 mini", input: 0.25, output: 2, reasoning: true, latency: 540, speed: 9 },
  { value: "claude-haiku-4-5", provider: "anthropic", name: "Claude Haiku 4.5", input: 1, output: 5, reasoning: false, latency: 360, speed: 11 },
  { value: "gpt-4.1-nano", provider: "openai", name: "GPT-4.1 nano", input: 0.1, output: 0.4, reasoning: false, latency: 280, speed: 12 },
];

export const instructions =
  "You write support replies for Northwind’s checkout team. Be warm, brief and specific. Never promise a refund the policy doesn’t allow.";

export const prompts = [
  "Order #4471 shipped three days late because of a carrier delay. Draft a short apology to the customer, and suggest a subject line.",
  "Good. Now return it as JSON with `subject` and `body`, and keep the body under 60 words.",
];

interface Canned {
  reasoning?: string;
  text: string;
}

export const answers: Record<string, Canned[]> = {
  "gpt-5": [
    {
      reasoning:
        "**Checking the facts**\n\nThe order is #4471 and the delay was the carrier’s, three days. Nothing says the package is damaged, so a refund isn’t in scope.\n\n**Choosing the tone**\n\nWarm and brief: own the delay, give the new date, offer something proportionate — free shipping on the next order fits the policy.",
      text: "**Subject:** Your order #4471 is on its way — and we’re sorry it’s late\n\nHi Maya,\n\nYour order #4471 left our warehouse on time, but a delay with our carrier held it up for three days. That’s not the experience we want for you, and I’m sorry.\n\nIt’s moving again and should arrive **by Thursday**. As a thank-you for your patience, your next order ships free — the code is applied to your account automatically.\n\nWarmly,\nThe Northwind team",
    },
    {
      reasoning:
        "**Shortening the body**\n\nKeep the apology, the new date and the free shipping; drop the second sentence about the warehouse. Count: 46 words.",
      text: '```json\n{\n  "subject": "Your order #4471 is on its way — and we’re sorry it’s late",\n  "body": "Hi Maya, a carrier delay held up order #4471 for three days, and we’re sorry. It’s moving again and should arrive by Thursday. Your next order ships free — the code is already on your account. Warmly, the Northwind team"\n}\n```',
    },
  ],
  "claude-sonnet-4-5": [
    {
      reasoning:
        "**Understanding the situation**\n\nA three-day carrier delay on order #4471. The customer needs to know what happened, when it arrives, and that we noticed.\n\n**Drafting**\n\nI’ll keep it to three short paragraphs and suggest two subject lines so the team can choose.",
      text: "Here’s a reply you can send as is:\n\n> Hi Maya,\n>\n> I’m sorry your order #4471 is running late. Our carrier had a delay that held it for three days — it’s back on the road and should reach you **Thursday**.\n>\n> We’ve added free shipping to your next order. Thank you for bearing with us.\n>\n> — Northwind support\n\n**Subject lines**\n\n1. Order #4471: a short delay, and our apology\n2. Your Northwind order is on its way",
    },
    {
      reasoning: "**Formatting as JSON**\n\nUse the first subject line. The body loses the greeting line break; 41 words.",
      text: '```json\n{\n  "subject": "Order #4471: a short delay, and our apology",\n  "body": "Hi Maya, I’m sorry order #4471 is running late. A carrier delay held it for three days; it should reach you Thursday. We’ve added free shipping to your next order. Thank you for bearing with us. — Northwind support"\n}\n```\n\nThe body is 41 words.',
    },
  ],
  "gpt-5-mini": [
    {
      reasoning: "**Plan**\n\nApologize, state the cause and the new date, add a small gesture. Keep it short.",
      text: "**Subject:** Sorry — order #4471 is running a little late\n\nHi Maya,\n\nWe’re sorry: a carrier delay held your order #4471 for three days. It’s on its way again and should arrive by Thursday.\n\nYour next order ships free on us.\n\nBest,\nNorthwind",
    },
    {
      reasoning: "**Convert**\n\nSame content, one string for the body.",
      text: '```json\n{\n  "subject": "Sorry — order #4471 is running a little late",\n  "body": "Hi Maya, we’re sorry: a carrier delay held order #4471 for three days. It should arrive by Thursday, and your next order ships free on us. Best, Northwind"\n}\n```',
    },
  ],
  "claude-haiku-4-5": [
    {
      text: "**Subject:** We’re sorry your order is late\n\nHi Maya,\n\nOrder #4471 was delayed three days by our carrier. It’s now on the way and should arrive Thursday. We’re sorry for the wait — your next shipping is on us.\n\nNorthwind support",
    },
    {
      text: '```json\n{\n  "subject": "We’re sorry your order is late",\n  "body": "Hi Maya, order #4471 was delayed three days by our carrier. It should arrive Thursday. Sorry for the wait — your next shipping is on us. Northwind support"\n}\n```',
    },
  ],
  "gpt-4.1-nano": [
    {
      text: "Subject: Apology for the delay of order #4471\n\nDear customer, we apologize for the delay of your order #4471 due to a carrier issue. It will arrive soon. Thank you for your patience.",
    },
    {
      text: '```json\n{ "subject": "Apology for the delay of order #4471", "body": "Dear customer, we apologize for the delay of your order #4471 due to a carrier issue. It will arrive soon." }\n```',
    },
  ],
};

export const tokensOf = (text: string) => Math.max(1, Math.round(text.length / 4));

export function costOf(model: PlaygroundModel, input: number, output: number): string {
  return ((input * model.input + output * model.output) / 1_000_000).toFixed(6);
}
