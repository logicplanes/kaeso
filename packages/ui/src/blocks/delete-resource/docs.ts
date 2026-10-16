import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  DeleteResource: {
    description: "Confirms deleting a resource: names the object and the consequence, asks for its name typed, and ends with a danger button.",
    props: {
      confirm: "Asks to type the resource's name (`typed`) or just to confirm (`simple`).",
      consequence: 'What happens: "Applications using it will stop authenticating with the gateway."',
      inline: "Renders in place, without a trigger, for review pages and tests.",
      kind: '"project", "key", "organization"',
      name: "The resource's name; typed to confirm.",
      onConfirm: "Called when the action is confirmed; return a promise to keep the dialog busy until it ends.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      trigger: "The element that opens it.",
      verb: '"Delete", "Archive". Defaults to "Delete" in the provider\'s language.',
    },
  },
} satisfies ComponentDocs;
