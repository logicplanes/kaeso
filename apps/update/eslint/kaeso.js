const FORBIDDEN = [
  /^(bg|from|via|to|fill|stroke|accent|caret|decoration|placeholder|divide|ring|outline|shadow|drop-shadow|border|rounded|opacity|blur|backdrop|font|leading|tracking|indent|underline|overline|line-through|no-underline|italic|not-italic|uppercase|lowercase|capitalize|normal-case|antialiased|subpixel-antialiased|tabular-nums|proportional-nums|transition|duration|ease|delay|animate|cursor|select|scale|rotate|skew|translate|filter|mix-blend|bg-blend)(-|$)/,
  /^(p|px|py|pt|pr|pb|pl|ps|pe)-/,
  /^(h|min-h|max-h|size)-/,
];
const TEXT_LAYOUT = /^text-(left|center|right|start|end|justify|wrap|nowrap|balance|pretty|ellipsis|clip)$/;
const HEIGHT_LAYOUT = /^(h-full|min-h-0|min-h-full|h-dvh|min-h-dvh)$/;

const SCALE = new Set([
  0, 1, 2, 3, 4, 5, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 34, 36, 40, 44, 48, 52, 56, 60, 64, 72, 80, 96, 120, 140, 160, 180, 200, 220, 240, 260, 280,
  320, 360, 440, 480, 560, 640, 720, 800,
]);
const SPACED = /^(m|mx|my|mt|mr|mb|ml|ms|me|w|min-w|max-w|gap|gap-x|gap-y|space-x|space-y|top|right|bottom|left|inset|inset-x|inset-y|basis)-(\d+)$/;

function offScale(cls) {
  const m = SPACED.exec(cls);
  return !!m && !SCALE.has(Number(m[2]));
}

function utilities(value) {
  return value
    .split(/\s+/)
    .filter(Boolean)
    .map((cls) => cls.replace(/^!/, "").split(":").pop() ?? cls)
    .map((cls) => cls.replace(/^-/, ""));
}

function visualUtility(cls) {
  if (TEXT_LAYOUT.test(cls) || HEIGHT_LAYOUT.test(cls)) return false;
  if (/^text-/.test(cls)) return true;
  return FORBIDDEN.some((re) => re.test(cls));
}

function stringsIn(node) {
  if (!node) return [];
  switch (node.type) {
    case "Literal":
      return typeof node.value === "string" ? [{ node, value: node.value }] : [];
    case "TemplateLiteral":
      return [...node.quasis.map((q) => ({ node: q, value: q.value.cooked ?? "" })), ...node.expressions.flatMap(stringsIn)];
    case "JSXExpressionContainer":
      return stringsIn(node.expression);
    case "ConditionalExpression":
      return [...stringsIn(node.consequent), ...stringsIn(node.alternate)];
    case "LogicalExpression":
      return [...stringsIn(node.left), ...stringsIn(node.right)];
    case "CallExpression":
      return node.arguments.flatMap(stringsIn);
    case "ArrayExpression":
      return node.elements.flatMap(stringsIn);
    case "ObjectExpression":
      return node.properties.flatMap((p) => (p.type === "Property" && p.key ? stringsIn(p.key.type === "Literal" ? p.key : null) : []));
    default:
      return [];
  }
}

const layoutOnly = {
  meta: {
    type: "problem",
    docs: { description: "className may only hold layout utilities; the library owns every visual decision." },
    schema: [],
  },
  create(context) {
    return {
      JSXAttribute(node) {
        const name = node.name?.name;
        if (name === "style") {
          context.report({ node, message: "No inline styles: compose @logicplanes/kaeso-ui components instead." });
          return;
        }
        if (name !== "className") return;
        for (const { node: n, value } of stringsIn(node.value)) {
          for (const cls of utilities(value)) {
            if (visualUtility(cls)) {
              context.report({
                node: n,
                message: `"${cls}" sets a visual style. className is for layout only (margin, width, grid, flex, gap, position); the look comes from @logicplanes/kaeso-ui.`,
              });
            } else if (offScale(cls)) {
              context.report({
                node: n,
                message: `"${cls}" is off the spacing scale, so it has no effect. Use a value from @logicplanes/kaeso-ui's --spacing-* tokens.`,
              });
            }
          }
        }
      },
    };
  },
};

const COLOR = /#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})(?![\w-])|\b(rgba?|hsla?|oklch|oklab|lab|lch)\(/;
const noColor = {
  meta: { type: "problem", docs: { description: "No color literals in the site's code." }, schema: [] },
  create(context) {
    const check = (node, value) => {
      if (typeof value === "string" && COLOR.test(value)) {
        context.report({ node, message: "Color literal in the site's code: colors come from @logicplanes/kaeso-ui tokens." });
      }
    };
    return {
      Literal: (node) => check(node, node.value),
      TemplateElement: (node) => check(node, node.value.cooked),
    };
  },
};

export default { rules: { "layout-only": layoutOnly, "no-color": noColor } };
