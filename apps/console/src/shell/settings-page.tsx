import type { ReactNode } from "react";
import { AppFrame, SettingsGroup, SettingsRow, SettingsSection, Skeleton } from "@logicplanes/kaeso-ui";
import { usePageTitle } from "./title";

export function SettingsPage({
  title,
  context,
  description,
  actions,
  children,
}: {
  title: string;
  context?: string;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
}) {
  usePageTitle(title, context);
  return (
    <AppFrame.Settings title={title} {...(description ? { description } : {})} {...(actions ? { actions } : {})}>
      {children}
    </AppFrame.Settings>
  );
}

export function SettingsLoading({ rows = 3 }: { rows?: number }) {
  return (
    <SettingsSection>
      <SettingsGroup>
        {Array.from({ length: rows }, (_, i) => (
          <SettingsRow key={i} title={<Skeleton shape="line" className="my-4 w-120" />} description={<Skeleton shape="line" className="w-240" />} />
        ))}
      </SettingsGroup>
    </SettingsSection>
  );
}
