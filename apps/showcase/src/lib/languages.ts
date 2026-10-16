import { matchUiLocale, UI_LOCALE_NAMES } from "@logicplanes/kaeso-ui/i18n";

export const DOCS_LANGUAGES = [
  { value: "en", label: UI_LOCALE_NAMES.en },
  { value: "de", label: UI_LOCALE_NAMES.de },
  { value: "zh-CN", label: UI_LOCALE_NAMES["zh-CN"] },
] as const;
export type DocsLang = (typeof DOCS_LANGUAGES)[number]["value"];

export const DOCS_LANG_STORAGE_KEY = "kaeso:docs-lang";

export function parseDocsLang(value: string | null | undefined): DocsLang | undefined {
  return matchUiLocale(value ?? "");
}

export function docsLangOf(param: string | undefined): DocsLang {
  return DOCS_LANGUAGES.find((l) => l.value === param)?.value ?? "en";
}

export function localizedPath(lang: DocsLang, path: string): string {
  if (lang === "en") return path;
  return path === "/" ? `/${lang}` : `/${lang}${path}`;
}

export function pageOfPath(pathname: string): { lang: DocsLang; path: string } {
  const clean = pathname.replace(/\.html$/, "").replace(/\/+$/, "") || "/";
  const m = /^\/(de|zh-CN)(?=\/|$)/.exec(clean);
  return m ? { lang: docsLangOf(m[1]), path: clean.slice(m[0].length) || "/" } : { lang: "en", path: clean };
}

export function languagePaths() {
  return DOCS_LANGUAGES.map((l) => ({ params: { lang: l.value === "en" ? undefined : l.value } }));
}

export function otherLanguagePaths() {
  return DOCS_LANGUAGES.filter((l) => l.value !== "en").map((l) => ({ params: { lang: l.value } }));
}

export function docsLangScript(page: DocsLang, notFound: boolean): string {
  return `(function(P,N){function p(v){v=(v||"").toLowerCase();if(v==="en"||v.indexOf("en-")===0)return"en";if(v==="de"||v.indexOf("de-")===0)return"de";if(v==="zh"||v.indexOf("zh-")===0)return"zh-CN";return""}var m=/^\\/(de|zh-CN)(?=\\/|$)/.exec(location.pathname),rest=m?location.pathname.slice(m[0].length)||"/":location.pathname,s=new URLSearchParams(location.search),q=p(s.get("lang")),w=q;if(q){s.delete("lang");try{localStorage.setItem("${DOCS_LANG_STORAGE_KEY}",q)}catch(e){}}if(!w&&m)w=m[1];if(!w){try{w=p(localStorage.getItem("${DOCS_LANG_STORAGE_KEY}"))}catch(e){}}if(!w){var n=navigator.languages&&navigator.languages.length?navigator.languages:[navigator.language];for(var i=0;i<n.length&&!w;i++)w=p(n[i])}w=w||"en";var qs=s.toString(),tail=(qs?"?"+qs:"")+location.hash;if(w!==P){location.replace((N?(w==="en"?"/404":"/"+w+"/404"):w==="en"?rest:"/"+w+(rest==="/"?"":rest))+tail)}else if(q){history.replaceState(history.state,"",location.pathname+tail)}})(${JSON.stringify(page)},${notFound});`;
}
