import ts from "typescript";

const isComponentName = (name) => /^[A-Z]/.test(name);

function rendersJsx(node) {
  let found = false;
  const visit = (n) => {
    if (found) return;
    if (ts.isJsxElement(n) || ts.isJsxSelfClosingElement(n) || ts.isJsxFragment(n)) found = true;
    else ts.forEachChild(n, visit);
  };
  visit(node);
  return found;
}

const isFunction = (node) => ts.isArrowFunction(node) || ts.isFunctionExpression(node);
const hasExport = (node) => !!node.modifiers?.some((m) => m.kind === ts.SyntaxKind.ExportKeyword);

export function componentsWithoutRef(fileName, source) {
  const file = ts.createSourceFile(fileName, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const line = (node) => file.getLineAndCharacterOfPosition(node.getStart(file)).line + 1;
  const plain = new Map();
  const exported = new Set();
  const found = [];
  const inline = (fn, name) => {
    if (rendersJsx(fn)) found.push({ name: fn.name?.text ?? name, line: line(fn) });
  };
  const exposes = (expr, name) => {
    if (ts.isIdentifier(expr)) exported.add(expr.text);
    else if (isFunction(expr)) inline(expr, name);
    else if (ts.isCallExpression(expr) && expr.expression.getText(file) === "Object.assign") {
      const [base, members] = expr.arguments;
      if (base) exposes(base, name);
      if (members && ts.isObjectLiteralExpression(members)) {
        for (const p of members.properties) {
          if (ts.isShorthandPropertyAssignment(p)) exported.add(p.name.text);
          else if (ts.isPropertyAssignment(p)) exposes(p.initializer, `${name}.${p.name.getText(file)}`);
        }
      }
    }
  };
  for (const statement of file.statements) {
    if (ts.isFunctionDeclaration(statement) && statement.name && isComponentName(statement.name.text) && statement.body && rendersJsx(statement.body)) {
      plain.set(statement.name.text, line(statement));
      if (hasExport(statement)) exported.add(statement.name.text);
    } else if (ts.isVariableStatement(statement)) {
      for (const d of statement.declarationList.declarations) {
        if (!ts.isIdentifier(d.name) || !d.initializer || !isComponentName(d.name.text)) continue;
        if (isFunction(d.initializer)) {
          if (rendersJsx(d.initializer)) plain.set(d.name.text, line(d));
          if (hasExport(statement)) exported.add(d.name.text);
        } else if (hasExport(statement)) exposes(d.initializer, d.name.text);
      }
    } else if (ts.isExportDeclaration(statement) && !statement.moduleSpecifier && statement.exportClause && ts.isNamedExports(statement.exportClause)) {
      for (const e of statement.exportClause.elements) exported.add((e.propertyName ?? e.name).text);
    } else if (ts.isExportAssignment(statement)) {
      exposes(statement.expression, "default");
    }
  }
  for (const [name, at] of plain) if (exported.has(name)) found.push({ name, line: at });
  return found.sort((a, b) => a.line - b.line);
}
