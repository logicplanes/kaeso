import { Suspense, lazy, type ComponentProps } from "react";
import { AuthLayout } from "@logicplanes/kaeso-ui";

const PublicLegalLinks = lazy(() => import("./legal-links").then((m) => ({ default: m.PublicLegalLinks })));

export function PublicPage({ legal = true, ...props }: ComponentProps<typeof AuthLayout> & { legal?: boolean }) {
  return (
    <main className="relative">
      <AuthLayout {...props} />
      {legal && (
        <div className="absolute inset-x-16 bottom-24">
          <Suspense fallback={null}>
            <PublicLegalLinks />
          </Suspense>
        </div>
      )}
    </main>
  );
}
