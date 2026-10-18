import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { loadUiMessages } from "@logicplanes/kaeso-ui/i18n";
import { Root } from "./app";
import { localeOf, setLocale, t } from "./i18n";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

const tag = localeOf(navigator.languages.length ? navigator.languages : [navigator.language]);
setLocale(tag);
document.documentElement.lang = tag;
document.title = t("page.title", "Kaeso releases");
const ui = await loadUiMessages(tag);

createRoot(root).render(
  <StrictMode>
    <Root locale={tag} ui={ui} />
  </StrictMode>,
);
