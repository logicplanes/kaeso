import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { LinkProvider, ThemeProvider } from "@logicplanes/kaeso-ui";
import { queryClient } from "./query";
import { createAppRouter, loadPageChunks } from "./router";
import { RouterLink } from "./shell/router-link";
import { AppToaster } from "./shell/toaster";
import { apply, load, resolve, storedChoice } from "./i18n";
import { I18nProvider } from "./i18n/provider";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root");

const router = createAppRouter();
void loadPageChunks(router, window.location.pathname);

void load(resolve(storedChoice())).then((initial) => {
  apply(initial);
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <I18nProvider initial={initial}>
          <ThemeProvider>
            <LinkProvider component={RouterLink}>
              <RouterProvider router={router} />
            </LinkProvider>
            <AppToaster />
          </ThemeProvider>
        </I18nProvider>
      </QueryClientProvider>
    </StrictMode>,
  );
});
