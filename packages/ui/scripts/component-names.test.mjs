import ts from "typescript";
import { describe, expect, it } from "vitest";
import { objectAssignName } from "./component-names.mjs";

function nameOf(code, exported) {
  const source = ts.createSourceFile("parts.tsx", code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const host = ts.createCompilerHost({});
  const program = ts.createProgram(
    ["parts.tsx"],
    { jsx: ts.JsxEmit.ReactJSX },
    { ...host, getSourceFile: (name) => (name === "parts.tsx" ? source : host.getSourceFile(name, ts.ScriptTarget.Latest)) },
  );
  const checker = program.getTypeChecker();
  const symbol = checker.getExportsOfModule(checker.getSymbolAtLocation(program.getSourceFile("parts.tsx")))?.find((s) => s.getName() === exported);
  return objectAssignName(symbol, program.getSourceFile("parts.tsx"));
}

describe("objectAssignName", () => {
  it("names an Object.assign export after the component it extends", () => {
    const code = `const Root = () => null;\nRoot.displayName = "Menu";\nconst Item = () => null;\nItem.displayName = "Menu.Item";\nexport const Menu = Object.assign(Root, { Item });\n`;
    expect(nameOf(code, "Menu")).toBe("Menu");
  });

  it("names a function component after its displayName, which the parser ignores for functions", () => {
    const code = `export function Provider() { return null; }\nProvider.displayName = "Sidebar.Provider";\n`;
    expect(nameOf(code, "Provider")).toBe("Sidebar.Provider");
  });

  it("leaves any other export to the parser", () => {
    expect(nameOf(`export function Plain() { return null; }\n`, "Plain")).toBeUndefined();
    expect(nameOf(`export const Value = 1;\n`, "Value")).toBeUndefined();
  });
});
