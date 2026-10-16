import { Suspense, lazy, useEffect, useState } from "react";
import { onRecentSignInChange, settleRecentSignIn } from "./recent-sign-in-store";

const RecentSignInDialog = lazy(() => import("./recent-sign-in-dialog"));

export function RecentSignInHost() {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    const stop = onRecentSignInChange(setOpen);
    return () => {
      stop();
      settleRecentSignIn(false);
    };
  }, []);
  if (!open) return null;
  return (
    <Suspense fallback={null}>
      <RecentSignInDialog />
    </Suspense>
  );
}
