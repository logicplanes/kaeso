import type { ReactNode } from "react";
import { cn } from "../../utils/cn";
import { AppFrame } from "../../components/app-frame/app-frame";
import { PageHeader, type PageHeaderProps } from "../../components/page-header/page-header";
import { Empty, type EmptyProps } from "../../components/empty/empty";
import { AuthLayout } from "../../components/section/section";
import { SplitView } from "../../components/section/section";

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
      <AppFrame.Body>{children}</AppFrame.Body>
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
  controls?: ReactNode;
  chart?: ReactNode;
  children?: ReactNode;
  aside?: ReactNode;
}) {
  const main = (
    <div className="flex flex-col gap-40">
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
      <AppFrame.Body>
        <div className="px-24 pt-24 pb-64">{aside ? <SplitView aside={aside}>{main}</SplitView> : main}</div>
      </AppFrame.Body>
    </>
  );
}
OverviewPage.displayName = "OverviewPage";

export function EmptyPage({ header, ...empty }: { header?: PageHeaderProps } & EmptyProps) {
  return (
    <>
      {header && <PageHeader start={<AppFrame.MenuButton />} {...header} />}
      <AppFrame.Body>
        <div className={cn("flex min-h-full items-center justify-center px-24")}>
          <Empty {...empty} />
        </div>
      </AppFrame.Body>
    </>
  );
}
EmptyPage.displayName = "EmptyPage";

export const AuthPage = AuthLayout;
