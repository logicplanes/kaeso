import type { ComponentDocs } from "../../component-docs.js";

export const docs = {
  DeleteResource: {
    description:
      "Confirms deleting a resource for good: the title names it, the body says the consequence, the person types its name, and a danger button names the action. A reversible action needs no confirmation: act at once and offer Undo in a toast.",
    props: {
      confirm: "Asks to type the resource's name (`typed`) or just to confirm (`simple`).",
      consequence: 'What happens: "Applications using it will stop authenticating with the gateway."',
      inline: "Renders in place, without a trigger, for review pages and tests.",
      kind: 'The kind of resource in the provider\'s language, as it reads after the verb on the button: "project", "key"; in German "Projekt".',
      name: "The resource's name: the title names it, and the person types it to confirm.",
      onConfirm: "Called when the action is confirmed; return a promise to keep the dialog busy until it ends.",
      onOpenChange: "Called when it opens or closes.",
      open: "Whether it is open, when you control it.",
      trigger: "The element that opens it.",
      verb: 'The action in the provider\'s language, in the form it takes on the button and in the title: "Archive" ("Archive project", "Archive “Checkout”?"); in German the infinitive, "archivieren" ("Projekt archivieren"). Defaults to "Delete" in the provider\'s language.',
    },
  },
} satisfies ComponentDocs;
