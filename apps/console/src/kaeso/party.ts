import { t } from "~/i18n";

export type PartyRef = { kind: string; id?: string | null | undefined };

export interface PartyLabel {
  name: string;
  you: boolean;
  avatarUrl?: string | undefined;
}

export const userLabel = ({ name, avatar_url }: { name: string; avatar_url?: string | null | undefined }, you = false): PartyLabel => ({
  name,
  you,
  avatarUrl: avatar_url ?? undefined,
});

export const partyText = ({ name, you }: PartyLabel): string => (you ? t("kaeso.party.you", "{name} (you)", { name }) : name);
