const visible = (text) =>
  text
    .replace(/`[^`]*`/g, " ")
    .replace(/\{[^{}]*\}/g, " ")
    .replace(/\{[^{}]*\}/g, " ");

export function translationProblem(lang, english, translated) {
  if (translated === undefined) return "missing";
  const text = visible(translated);
  if (translated === english && (text.match(/[A-Za-z]+/g) ?? []).length > 1) return "still the English";
  if (lang === "de" && /(^|[^\p{L}])(du|dich|dir|dein\p{L}*)(?!\p{L})/iu.test(text)) return "says „du“; address the reader with „Sie“";
  if (lang === "zh-CN" && text.includes("你")) return "says 你; address the reader with 您";
  return null;
}
