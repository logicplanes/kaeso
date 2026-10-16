import type { ComponentProps } from "react";
import { AuthLayout } from "@logicplanes/kaeso-ui";

export function PublicPage(props: ComponentProps<typeof AuthLayout>) {
  return (
    <main>
      <AuthLayout {...props} />
    </main>
  );
}
