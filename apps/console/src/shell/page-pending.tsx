import { AppFrame, PageHeader, Skeleton } from "@logicplanes/kaeso-ui";

export function PagePending() {
  return (
    <>
      <PageHeader start={<AppFrame.MenuButton />} />
      <AppFrame.Body padding="page">
        <Skeleton className="h-full w-full" />
      </AppFrame.Body>
    </>
  );
}
