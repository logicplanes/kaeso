import { queryOptions } from "@tanstack/react-query";
import { api, data } from "../client";
import type { WebhookView } from "../generated/types";

export const webhooksKey = (org: string) => ["orgs", org, "webhooks"];

export const webhooksQuery = (org: string) =>
  queryOptions({
    queryKey: webhooksKey(org),
    queryFn: async (): Promise<WebhookView[]> => data(await api.GET("/platform/v1/orgs/{org}/webhooks", { params: { path: { org } } })).data,
  });
