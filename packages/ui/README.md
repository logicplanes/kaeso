# @logicplanes/kaeso-ui

Kaeso's UI library: React 19 components on Base UI, styled with Tailwind CSS v4 and the Kaeso tokens. Every Kaeso interface is built from it. Its showcase at [ui.kaeso.app](https://ui.kaeso.app) shows every component live, with its props.

## Install

```bash
pnpm add @logicplanes/kaeso-ui react react-dom @hugeicons/react @hugeicons/core-free-icons
```

With Tailwind CSS v4:

```css
@import "tailwindcss";
@import "@logicplanes/kaeso-ui/tailwind.css";
```

Without Tailwind, import the prebuilt stylesheet once: `import "@logicplanes/kaeso-ui/styles.css";`

```tsx
import { ThemeProvider, TooltipProvider, Toaster, Button } from "@logicplanes/kaeso-ui";

export function App() {
  return (
    <ThemeProvider>
      <TooltipProvider>
        <Button variant="primary">Create key</Button>
        <Toaster />
      </TooltipProvider>
    </ThemeProvider>
  );
}
```

Installation, theming and every component: [ui.kaeso.app](https://ui.kaeso.app).

## For agents

The package ships its rules in `AGENTS.md`. Load them in your project so every agent follows them: add `@node_modules/@logicplanes/kaeso-ui/AGENTS.md` to your `CLAUDE.md`, or tell agents in your `AGENTS.md` to read `node_modules/@logicplanes/kaeso-ui/AGENTS.md`. The registry of every component and prop is in `@logicplanes/kaeso-ui/ai/component-registry.md` (and `.json`); online as [ui.kaeso.app/components.json](https://ui.kaeso.app/components.json), and with the rules as plain text at [ui.kaeso.app/llms.txt](https://ui.kaeso.app/llms.txt).

## License

MIT. The Kaeso name and logo are not covered by the license. Inter and Geist Mono are bundled under the SIL Open Font License 1.1; see `NOTICE.md`.
