import { Suspense, lazy, useEffect, useState } from "react";

const CreateOrgDialog = lazy(() => import("./create-org-dialog"));

const listeners = new Set<() => void>();

export function openCreateOrg(): void {
  for (const l of listeners) l();
}

// Outside the sidebar, so the dialog stays open when the phone's navigation sheet that offered it closes.
export function CreateOrgHost() {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const show = () => setOpen(true);
    listeners.add(show);
    return () => void listeners.delete(show);
  }, []);
  if (!open) return null;
  return (
    <Suspense fallback={null}>
      <CreateOrgDialog onClose={() => setOpen(false)} />
    </Suspense>
  );
}
