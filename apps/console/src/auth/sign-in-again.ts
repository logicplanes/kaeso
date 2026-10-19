import type { QueryClient } from "@tanstack/react-query";
import { requestRecentSignIn } from "./recent-sign-in-store";
import { sessionQuery } from "./session";

export async function signInAnotherWay(client: QueryClient): Promise<void> {
  if (await requestRecentSignIn("without-sso")) await client.invalidateQueries({ queryKey: sessionQuery.queryKey });
}
