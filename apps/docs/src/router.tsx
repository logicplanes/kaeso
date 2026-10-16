import { Outlet, createRootRoute, createRoute, createRouter, type AnyRoute, type RouterHistory } from "@tanstack/react-router";
import { DEFAULT_LOCALE, LOCALES, localePrefix, type Locale } from "~/i18n/locales";
import { DocPage } from "~/page/doc-page";
import { loadPage, pagePath } from "~/page/load";
import { NotFound } from "~/page/not-found";
import { BootError, PageError } from "~/page/page-error";
import { LocaleShell } from "~/shell/locale-shell";
import { loadLocaleData } from "~/site/data";

function localeLayout(root: AnyRoute, locale: Locale) {
  const options = {
    getParentRoute: () => root,
    beforeLoad: async () => ({ data: await loadLocaleData(locale) }),
    errorComponent: BootError,
  };
  return locale === DEFAULT_LOCALE ? createRoute({ ...options, id: locale }) : createRoute({ ...options, path: locale });
}

function localeRoutes(root: AnyRoute, locale: Locale) {
  const layout = localeLayout(root, locale);
  layout.update({
    component: function Layout() {
      return <LocaleShell data={layout.useRouteContext().data} />;
    },
  });
  const prefix = localePrefix(locale);
  const page = (path: "/" | "$") => {
    const route = createRoute({
      getParentRoute: () => layout,
      path,
      loader: ({ context, params }) => loadPage(context.data.site, pagePath(prefix, "_splat" in params ? params._splat : undefined)),
      errorComponent: PageError,
      notFoundComponent: NotFound,
    });
    route.update({
      component: function Page() {
        return <DocPage {...route.useLoaderData()} />;
      },
    });
    return route;
  };
  return layout.addChildren([page("/"), page("$")]);
}

export function createDocsRouter(history?: RouterHistory) {
  const root = createRootRoute({ component: Outlet });
  return createRouter({
    routeTree: root.addChildren(LOCALES.map((locale) => localeRoutes(root, locale))),
    defaultPreload: "intent",
    defaultStaleTime: Infinity,
    defaultPreloadStaleTime: Infinity,
    trailingSlash: "never",
    defaultRemountDeps: ({ params }) => params,
    ...(history ? { history } : {}),
  });
}
