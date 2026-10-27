# AGENTS.md

Rules for working on the UI library: `@logicplanes/kaeso-tokens` in `packages/tokens` and `@logicplanes/kaeso-ui` in `packages/ui`. The root [`AGENTS.md`](../AGENTS.md) holds the setup, the definition of done and the rules for the whole repository; the showcase has its own in [`apps/showcase/AGENTS.md`](../apps/showcase/AGENTS.md).

`packages/ui/AGENTS.md` is a different file: the rules for apps that build interfaces with the library, shipped in the npm package and rendered at [ui.kaeso.app/for-agents](https://ui.kaeso.app/for-agents). Follow it too, and change it only when those rules change.

## What the library is

- **Is:** the only place Kaeso's interface is designed: tokens, components, blocks, formatters, translations, and the tests that keep them honest.
- **Is not:** an application. The console and the documentation site compose these components from the workspace and never style one of their own; a component or variant an app needs is added here, in the same pull request.

## Checks

The root definition of done covers the library. `pnpm build` builds both packages, checks the bundle budgets (`packages/ui/scripts/budgets.mjs`) and builds the showcase, whose page generator writes `packages/ui/ai/component-guides.md`; `pnpm release:check` packs both packages and checks the tarballs; `pnpm --filter kaeso-showcase e2e` runs the showcase's Playwright suite against its build. Then look at the changed component's `/components/<slug>/states` page in light and dark, at desktop and phone widths, in English, German and Chinese, and fix what looks wrong.

## Design rules

1. **Pure white and pure black.** Light mode on `#ffffff`, dark mode on `#000000`; text one step off pure.
2. **Monochrome interface.** Colour only in data and in status marks that sit next to a word or an icon. Never colour alone.
3. **Pills** for every single-line control; concentric radii for containers.
4. **Tokens only.** Components use role names (`bg-panel`, `text-fg-secondary`, `h-36`, `rounded-pill`). No hex or rgb literals, no arbitrary Tailwind values or variants, no `dark:`, no `border` on controls (rings are shadows), no length off the scale, no duration off the motion table. `pnpm lint` enforces it (`packages/ui/scripts/lint.mjs`). A finding that applies everywhere changes a token in `packages/tokens/src/tokens.ts`, or a base value in `packages/tokens/src/brand/tokens.json`, not a component.
5. **One gutter, boxes on it.** Every box (a control, a selected tab, a row's hover fill, a group) starts and ends on the gutter, 24px in the content panel at every width; text inside a box sits 12px further in. Nothing bleeds into the gutter through a negative margin. The rule and its exceptions are in `packages/ui/foundations/layout.md`.
6. **Nothing changes size when its state changes.** Labels that change (Copy → Copied, Save → Saving…) reserve every variant's width (`SwapText`, `reserveLabels`). Hover, focus, selection and loading never move a pixel.
7. **Focus rings for the keyboard only.** A clicked control never keeps a ring.
8. **No decorative cards, icons or dots.** Content sits on the panel; empty states, dialogs, badges and status labels carry words.
9. **Never a serif.** Every control and portal inherits Inter; code uses Geist Mono.
10. **The wordmark alone in interfaces.** `Logo.Wordmark size="nav"`, top left, 28px in and 28px down in the sidebar and on sign-in screens; on the phone bar it sits on the page's 24px gutter, and on a phone's sign-in screen on the form's 16px gutter. The mark is for favicons, app icons and avatars; the lockup for artwork outside interfaces.
11. **Every component:** `forwardRef` with a `displayName`; `KAESO_<NAME>_VARIANTS` (classes and a description per key) and `KAESO_<NAME>_DEFAULT_VARIANTS`; optional props typed `?: T | undefined`; an accessible name required where there is no visible label; forced states through `data-force` for the `/states` pages.
12. **Every component is described in data.** Each component folder has a `docs.ts` with the component's description and one sentence per prop, written so someone knows when to pass it; an empty string uses the shared description in `packages/ui/src/component-docs.ts`. The registry, the showcase's prop tables and the agent guides are generated from it, and the build fails when a description is missing.
13. **Overlays portal into the nearest `PortalContainer`** (themed regions, previews, persisted islands) and position against the window, so a popup flips, shifts and scrolls inside the viewport at any zoom.
14. **Copy follows the voice:** sentence case, a verb first on buttons, no exclamation points or emoji, errors that say what to do next. Examples use realistic Kaeso data, never lorem ipsum, and describe the console in Kaeso's own terms.
15. **Every word in three languages.** Library text lives beside its key (`t("dialog.close", "Close")`) with German and Simplified Chinese in `packages/ui/src/i18n/locales`; tests check that every key exists in all three.
16. **No setup checklists.** The library has no setup checklist or numbered steps component and never gets one back. A new project shows its normal screens with their `Empty` states; a task in several steps is a `StepDialog`.
17. **Coverage thresholds** live in each package's `vitest.config.ts`.
18. **Versions:** both packages always share Kaeso's version. A change to how a component looks that keeps its API is minor; an API change is major.

## Where things are

| Path                                                | Holds                                                                                                            |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `packages/tokens/src/brand/tokens.json`             | The brand's base values: colours, contrast, type, radii, motion, controls, spacing, the logo and icons           |
| `packages/tokens/src/tokens.ts`                     | Every role token, built from the base values in `brand/tokens.json`                                              |
| `packages/ui/src/components/<name>/`                | One folder per entry point: the component, its `docs.ts` and its tests                                           |
| `packages/ui/src/blocks/<name>/`                    | One folder per block entry point (`@logicplanes/kaeso-ui/blocks/<name>`): the block, its `docs.ts` and its tests |
| `packages/ui/src/i18n/`                             | `LocaleProvider`, the message formatter and the English, German and Chinese catalogues                           |
| `packages/ui/src/styles/kaeso.css`                  | Variants for forced states, the control material, motion utilities                                               |
| `packages/ui/AGENTS.md`, `packages/ui/foundations/` | The rules for apps that use the library, shipped in the package                                                  |
| `packages/ui/ai/`                                   | Generated by the build: component registry, guides, `tokens.md`, `icons.md`; never edit                          |
| `packages/ui/scripts/`                              | The build steps, the bundle budgets and the token lint                                                           |
| `scripts/check-pack.mjs`, `scripts/licenses.mjs`    | The package checks and the licence check of both packages' dependencies                                          |
