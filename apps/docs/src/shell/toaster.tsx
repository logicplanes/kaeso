import { Suspense, lazy, useEffect } from "react";
import { markToasterMounted } from "./toast";

const LazyToaster = lazy(() =>
  import("@logicplanes/kaeso-ui/toast").then(({ Toaster }) => ({
    default: function MountedToaster() {
      useEffect(markToasterMounted, []);
      return <Toaster />;
    },
  })),
);

export function AppToaster() {
  return (
    <Suspense fallback={null}>
      <LazyToaster />
    </Suspense>
  );
}
