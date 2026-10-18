## What and why

## How it was tested

## Checklist

- [ ] The definition of done in `AGENTS.md` passes
- [ ] Tests cover the change, and no coverage threshold was lowered
- [ ] The title is the changelog's line: one sentence that says what changed for people who use, run or build on Kaeso; the labels choose its section and topic
- [ ] The pull request holds one change; one that builds on an unmerged pull request is stacked on it with `gh stack`
- [ ] `openapi.json` is regenerated and the console updated when the Platform API changed
- [ ] New wire fields have a fixture with provenance and a row on the dialect's mapping page
- [ ] New text and changed pages exist in English, German and Simplified Chinese
- [ ] Changed interfaces were checked in light and dark, at desktop and phone widths
- [ ] The documentation in `apps/docs/content` is updated, or no change is needed
