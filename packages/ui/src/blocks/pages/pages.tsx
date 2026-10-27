import type { ReactNode } from "react";
import { AppFrame } from "../../components/app-frame/app-frame.js";
import { PageHeader, type PageHeaderProps } from "../../components/page-header/page-header.js";
import { Empty, type EmptyProps } from "../../components/empty/empty.js";
import { AuthLayout } from "../../components/section/section.js";
import { SplitView } from "../../components/section/section.js";

export const KAESO_PAGES_VARIANTS = {
  page: {
    list: { classes: "", description: "Header, toolbar, a full-bleed List" },
    table: { classes: "", description: "Header, toolbar, a DataTable edge to edge" },
    overview: { classes: "", description: "A metric row, a chart, a breakdown and an optional aside" },
    empty: { classes: "", description: "Empty centered in the panel" },
    auth: { classes: "", description: "No chrome; a centered 400px column" },
  },
} as const;
export const KAESO_PAGES_DEFAULT_VARIANTS = { page: "list" } as const;

export interface ListPageProps {
  header: PageHeaderProps;
  toolbar?: ReactNode | undefined;
  children: ReactNode;
  sheet?: ReactNode | undefined;
}

export function ListPage({ header, toolbar, children, sheet }: ListPageProps) {
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} {...header} />
      {toolbar}
      <AppFrame.Body padding="list">{children}</AppFrame.Body>
      {sheet}
    </>
  );
}
ListPage.displayName = "ListPage";

export const TablePage = ListPage;

export function OverviewPage({
  header,
  metrics,
  controls,
  chart,
  children,
  aside,
}: {
  header: PageHeaderProps;
  metrics: ReactNode;
  controls?: ReactNode | undefined;
  chart?: ReactNode | undefined;
  children?: ReactNode | undefined;
  aside?: ReactNode | undefined;
}) {
  const main = (
    <div className="flex flex-col gap-48">
      <div className="flex flex-wrap items-end justify-between gap-16">
        {metrics}
        {controls && <div className="flex items-center gap-8">{controls}</div>}
      </div>
      {chart}
      {children}
    </div>
  );
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} {...header} />
      <AppFrame.Body padding="page">{aside ? <SplitView aside={aside}>{main}</SplitView> : main}</AppFrame.Body>
    </>
  );
}
OverviewPage.displayName = "OverviewPage";

export function EmptyPage({ header, ...empty }: { header?: PageHeaderProps | undefined } & EmptyProps) {
  return (
    <>
      {header && <PageHeader start={<AppFrame.MenuButton />} {...header} />}
      <AppFrame.Body className="relative">
        {!header && <AppFrame.MenuButton className="absolute top-10 left-24 z-10" />}
        <div className="flex min-h-full items-center justify-center px-24">
          <Empty {...empty} />
        </div>
      </AppFrame.Body>
    </>
  );
}
EmptyPage.displayName = "EmptyPage";

export const AuthPage = AuthLayout;
