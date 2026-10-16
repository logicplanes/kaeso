import { t } from "~/i18n";

export type PartyRef = { kind: string; id?: string | null | undefined };

export interface PartyLabel {
  name: string;
  you: boolean;
}

export const partyText = ({ name, you }: PartyLabel): string => (you ? t("kaeso.party.you", "{name} (you)", { name }) : name);
