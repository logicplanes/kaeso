# Writing

Clear enough to act on, exact enough to trust, and calm enough to stay out of the way. These are the rules that interface copy meets every day.

## Rules

- **Sentence case** everywhere: buttons, titles, menu items, column headers.
- **A verb first on buttons**: "Create key", "Add model", "Disable". Never "OK" or "Submit".
- **No exclamation points, no emoji**, no jokes, no congratulations.
- **Errors say what did not happen and what to do next**: "Couldn't reach the provider. Check the provider key, then try again."
- **Name the object**: "Key copied", not "Copied to clipboard".
- **Use Kaeso's terms**: key, provider key, project, organization, model, alias, limit, budget, request log.
- **Realistic data in examples**, never lorem ipsum.
- **Straight apostrophes** in English: "Couldn't load these rows", "the key's name", never `’`.

## Labels and placeholders

A label names the field; a placeholder shows an example, never the label. Hints say what the value is used for. Required fields are the default; optional ones say "Optional".

| Do                                    | Avoid                                     |
| ------------------------------------- | ----------------------------------------- |
| Name, with placeholder "checkout-web" | Placeholder "Enter key name" and no label |
| "Used in request logs and alerts."    | "Please enter a valid name."              |

## Empty states

An empty state says what will appear and how to make it appear, in one sentence, with one action. No illustration, no icon.

| Do                                                                | Avoid                  |
| ----------------------------------------------------------------- | ---------------------- |
| "No requests yet. Send a request with a key and it appears here." | "Nothing to see here!" |

## Destructive actions

Say the consequence in plain words before the action. The button repeats the verb: "Archive project", "Delete key". Reversible actions skip the confirmation and offer Undo in the toast.

## Numbers and units

Units sit next to numbers: `$1,284.50`, `18.4M tokens`, `840 ms`. Money keeps the decimals the API sent; an amount below the smallest place shown reads `< $0.000001`, never `$0.000000`. Counts round only for display, keep at least three significant digits (`1.23M`, `1.02亿`) and show the exact value in a tooltip.
