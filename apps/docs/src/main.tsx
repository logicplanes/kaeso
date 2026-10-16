import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { followPreferredLocale } from "./i18n/preference";
import { createDocsRouter } from "./router";
import { RouterLink } from "./shell/router-link";
import { AppToaster } from "./shell/toaster";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

followPreferredLocale(window);
const router = createDocsRouter();

createRoot(root).render(
  <StrictMode>
    <ThemeProvider>
      <LinkProvider component={RouterLink}>
        <RouterProvider router={router} />
      </LinkProvider>
      <AppToaster />
    </ThemeProvider>
  </StrictMode>,
);
