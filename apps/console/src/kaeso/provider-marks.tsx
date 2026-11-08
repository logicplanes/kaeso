import type { ReactNode } from "react";
import { ProviderMarksProvider } from "@logicplanes/kaeso-ui";
import { useProviders } from "~/kaeso/providers";

export function OrgProviderMarks({ org, children }: { org: string; children: ReactNode }) {
  const providers = useProviders(org);
  return <ProviderMarksProvider markOf={providers.mark}>{children}</ProviderMarksProvider>;
}
