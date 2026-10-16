import { createFileRoute } from "@tanstack/react-router";
import { AccountKeysPage } from "~/features/account/keys-page";

export const Route = createFileRoute("/_app/settings/account/keys")({ component: AccountKeysPage });
