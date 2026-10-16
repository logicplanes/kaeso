import type { APIRoute } from "astro";
import { registry } from "~/lib/registry";
import agents from "../../../../packages/ui/AGENTS.md?raw";

export const GET: APIRoute = () => {
  const rules = agents.replace(/^# .*\n+/, "");
  const lines = [
    "# Kaeso UI",
    "",
    "> Kaeso's React component library on Base UI, Tailwind CSS v4 and the Kaeso tokens. Pure white and pure black, pill controls, a monochrome interface.",
    "",
    "- Registry: https://ui.kaeso.app/components.json",
    "- Source: https://github.com/logicplanes/kaeso/tree/dev/packages/ui",
    "",
    rules.trim(),
    "",
    "## Components",
    "",
  ];
  for (const c of registry.components) {
    lines.push(`### ${c.entry}`, "", `import from \`${c.import}\``, "");
    for (const comp of c.components) {
      lines.push(`- **${comp.name}**${comp.description ? `: ${comp.description.replace(/\n/g, " ")}` : ""}`);
      for (const p of comp.props)
        lines.push(
          `  - \`${p.name}\`${p.required ? " (required)" : ""}: \`${String(p.type).replace(/\n/g, " ")}\`${p.default ? `, default \`${p.default}\`` : ""}${p.description ? ` — ${p.description.replace(/\n/g, " ")}` : ""}`,
        );
    }
    lines.push("");
  }
  return new Response(lines.join("\n"), { headers: { "Content-Type": "text/plain; charset=utf-8" } });
};
