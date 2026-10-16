import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  Logo: {
    description: "`<Logo />` is the lockup; the parts are `Logo.Mark`, `Logo.Icon`, `Logo.Lockup`, `Logo.Wordmark`.",
    props: {
      decorative: "Hides it from assistive technology when the same information is already written next to it.",
      size: "The logo's height.",
      title: "The accessible name. Pass `decorative` when the name is visible next to it.",
    },
  },
} satisfies ComponentDocs;
