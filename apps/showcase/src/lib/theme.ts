import { THEME_STORAGE_KEY } from "@logicplanes/kaeso-ui";

export const docsThemeSwapScript = `(function(){if(window.__kaesoDocsTheme)return;window.__kaesoDocsTheme=1;document.addEventListener("astro:after-swap",function(){var d=document.documentElement;try{var t=localStorage.getItem("${THEME_STORAGE_KEY}");if(t==="light"||t==="dark"){d.setAttribute("data-theme",t)}else{d.removeAttribute("data-theme")}}catch(e){}})})();`;
