import ts from "typescript";

// react-docgen-typescript names an export after the `.displayName` statement that follows it in the file, so
// `export const Menu = Object.assign(Root, …)` took the next component's name and ContextMenu was documented with Menu's props.
// An export built with Object.assign takes the displayName of the component it extends; anything else keeps the parser's name.
export function objectAssignName(symbol, source) {
  const declaration = symbol.valueDeclaration;
  if (!declaration || !ts.isVariableDeclaration(declaration)) return undefined;
  const init = declaration.initializer;
  if (!init || !ts.isCallExpression(init) || init.expression.getText(source) !== "Object.assign") return undefined;
  const base = init.arguments[0];
  if (!base || !ts.isIdentifier(base)) return undefined;
  for (const statement of source.statements) {
    if (!ts.isExpressionStatement(statement) || !ts.isBinaryExpression(statement.expression)) continue;
    const { left, right, operatorToken } = statement.expression;
    if (operatorToken.kind !== ts.SyntaxKind.EqualsToken || !ts.isPropertyAccessExpression(left) || !ts.isStringLiteral(right)) continue;
    if (left.name.text === "displayName" && ts.isIdentifier(left.expression) && left.expression.text === base.text) return right.text;
  }
  return undefined;
}
