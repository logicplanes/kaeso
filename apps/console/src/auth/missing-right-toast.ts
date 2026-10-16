import type { ApiError } from "~/api/errors";
import { queryClient } from "~/query";
import { toast } from "~/shell/toast";
import { explainMissingRight } from "./needs-right";

export function placeOf(pathname: string): { org: string | undefined; project: string | undefined } {
  const m = /^\/(?:settings\/)?orgs\/([^/]+)(?:\/projects\/([^/]+))?/.exec(pathname);
  return { org: m?.[1], project: m?.[2] };
}

export async function toastMissingRight(error: ApiError, options: { id?: string } = {}): Promise<void> {
  const { org, project } = placeOf(window.location.pathname);
  const text = org && error.right ? await explainMissingRight(queryClient, org, project, error.right) : error.message;
  toast.error(text, options);
}
