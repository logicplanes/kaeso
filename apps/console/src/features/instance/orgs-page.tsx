import { useState } from "react";
import { keepPreviousData, useInfiniteQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  Avatar,
  Badge,
  Button,
  Combobox,
  DataTable,
  Dialog,
  Empty,
  FilterRow,
  IconButton,
  Input,
  Menu,
  SearchInput,
  Text,
  col,
  icons,
  type SortingState,
} from "@logicplanes/kaeso-ui";
import { formatDate, formatNumber } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import type { InstanceOrgView } from "~/api/generated/types";
import { instanceOrgsQuery, instanceUsersQuery } from "~/api/queries/instance";
import { useApiMutation } from "~/auth/mutation";
import { useSession } from "~/auth/session";
import { usePageCreateAction } from "~/shell/create-action";
import { toast } from "~/shell/toast";
import { InstancePage } from "./instance-page";
import { DeleteNowDialog, useRestoreOrg } from "./org-deletion";
import { t } from "~/i18n";

export type InstanceOrgsSearch = { q?: string; sort?: "deletion"; create?: boolean };

export function InstanceOrgsPage({ search }: { search: InstanceOrgsSearch }) {
  const navigate = useNavigate();
  const [q, setQ] = useState(search.q ?? "");
  const session = useSession();
  const byDeletion = search.sort === "deletion";
  const [localSorting, setLocalSorting] = useState<SortingState>([]);
  const sorting: SortingState = byDeletion ? [{ id: "state", desc: false }] : localSorting;
  const orgs = useInfiniteQuery(instanceOrgsQuery(search.q ?? "", byDeletion ? "deletion" : "created"));
  const [creating, setCreating] = useState(!!search.create);
  const [deleting, setDeleting] = useState<InstanceOrgView | null>(null);
  const restore = useRestoreOrg();
  usePageCreateAction({ label: t("instance.createOrg", "Create organization"), run: () => setCreating(true) });
  const rows = orgs.data?.pages.flatMap((p) => p.data) ?? [];
  const show = (next: { q?: string | undefined; sort?: "deletion" | undefined }) =>
    void navigate({
      to: "/settings/instance/orgs",
      search: { ...(next.q ? { q: next.q } : {}), ...(next.sort ? { sort: next.sort } : {}) },
      replace: true,
    });
  const searchFor = (value: string) => show({ q: value.trim(), sort: search.sort });
  const sortBy = (next: SortingState) => {
    const state = next.find((s) => s.id === "state");
    const deletion = !!state && !state.desc;
    if (deletion !== byDeletion) show({ q: search.q, sort: deletion ? "deletion" : undefined });
    setLocalSorting(next.filter((s) => s.id !== "state"));
  };

  const columns = [
    col.custom(
      { id: "name", header: t("common.name", "Name"), size: 192, priority: 1, grow: true },
      (o: InstanceOrgView) => o.name,
      (_v, o) => (
        <span className="flex min-w-0 items-center gap-8">
          <Avatar name={o.name} {...(o.avatar_url ? { src: o.avatar_url } : {})} kind="org" size={20} decorative />
          <span className="truncate">{o.name}</span>
        </span>
      ),
    ),
    col.custom(
      { id: "state", header: t("instance.state", "State"), size: 168, priority: 1 },
      (o: InstanceOrgView) => (o.state === "scheduled_for_deletion" ? (o.deletion_due_at ?? "") : "~"),
      (_v, o) =>
        o.state === "scheduled_for_deletion" ? (
          <Badge tone="danger">
            {o.deletion_due_at
              ? t("instance.deletionOn", "Deletion on {date}", { date: thisYear(o.deletion_due_at) })
              : t("instance.scheduledForDeletion", "Scheduled for deletion")}
          </Badge>
        ) : o.owners === 0 ? (
          <Badge tone="warning">{t("instance.noOwner", "No owner")}</Badge>
        ) : (
          <Text>{t("common.active", "Active")}</Text>
        ),
    ),
    col.custom(
      { id: "members", header: t("common.members", "Members"), size: 104, priority: 2 },
      (o: InstanceOrgView) => o.members,
      (v: number) => formatNumber(v),
      { align: "end" },
    ),
    col.custom(
      { id: "projects", header: t("common.projects", "Projects"), size: 104, priority: 2 },
      (o: InstanceOrgView) => o.projects,
      (v: number) => formatNumber(v),
      { align: "end" },
    ),
    col.custom(
      { id: "created", header: t("common.created", "Created"), size: 112, priority: 3 },
      (o: InstanceOrgView) => o.created_at,
      (v: string) => <time dateTime={v}>{formatDate(v)}</time>,
    ),
  ];

  return (
    <InstancePage
      title={t("instance.orgs", "Organizations")}
      description={t("instance.orgsHint", "Every organization on this install: metadata only, not keys or logs.")}
      actions={
        <>
          <Button size="sm" variant="primary" leadingIcon={icons.add} className="hidden sm:inline-flex" onClick={() => setCreating(true)}>
            {t("instance.createOrg", "Create organization")}
          </Button>
          <IconButton
            size="sm"
            variant="primary"
            icon={icons.add}
            aria-label={t("instance.createOrg", "Create organization")}
            className="sm:hidden"
            onClick={() => setCreating(true)}
          />
        </>
      }
    >
      <FilterRow>
        <div className="w-240">
          <SearchInput
            aria-label={t("instance.searchOrgs", "Search organizations")}
            placeholder={t("common.name", "Name")}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onClear={() => {
              setQ("");
              searchFor("");
            }}
            onKeyDown={(e) => e.key === "Enter" && searchFor(q)}
            onBlur={() => q.trim() !== (search.q ?? "") && searchFor(q)}
          />
        </div>
      </FilterRow>
      <DataTable
        aria-label={t("instance.orgs", "Organizations")}
        data={rows}
        columns={columns}
        getRowId={(o) => o.id}
        loading={orgs.isPending}
        error={orgs.isError && rows.length === 0 ? t("instance.orgsFailed", "Organizations couldn't load") : undefined}
        onRetry={() => void orgs.refetch()}
        empty={
          <Empty
            size="md"
            title={search.q ? t("instance.noOrgsMatch", "No organizations match") : t("instance.noOrgs", "No organizations")}
            {...(search.q ? { description: t("instance.searchAnotherName", "Search by another name.") } : {})}
          />
        }
        hasMore={orgs.hasNextPage}
        loadingMore={orgs.isFetchingNextPage}
        onLoadMore={() => void orgs.fetchNextPage()}
        onRowClick={(o) => void navigate({ to: "/settings/instance/orgs/$org", params: { org: o.id } })}
        sorting={sorting}
        onSortingChange={sortBy}
        rowMenu={(o) =>
          o.state === "scheduled_for_deletion" ? (
            <>
              <Menu.Item icon={icons.refresh} onClick={() => restore.mutate(o.id)}>
                {t("instance.restoreOrg", "Restore organization")}
              </Menu.Item>
              {session.org_deletion.allowed && (
                <Menu.Item icon={icons.delete} variant="danger" onClick={() => setDeleting(o)}>
                  {t("instance.deleteNow", "Delete now")}
                </Menu.Item>
              )}
            </>
          ) : undefined
        }
      />
      {creating && (
        <CreateOrgDialog
          onClose={() => {
            setCreating(false);
            if (search.create) show({ q: search.q, sort: search.sort });
          }}
        />
      )}
      {deleting && <DeleteNowDialog org={deleting} onClose={() => setDeleting(null)} inTable />}
    </InstancePage>
  );
}

function thisYear(date: string): string {
  const year = new Date().getFullYear();
  return formatDate(date).replace(` ${year}`, "").replace(`${year}年`, "");
}

function CreateOrgDialog({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [owner, setOwner] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const users = useInfiniteQuery({ ...instanceUsersQuery(query, "active"), placeholderData: keepPreviousData });
  const options = (users.data?.pages.flatMap((p) => p.data) ?? []).map((u) => ({
    value: u.id,
    label: u.name,
    description: u.email ?? undefined,
    leading: <Avatar name={u.name} {...(u.avatar_url ? { src: u.avatar_url } : {})} size={20} decorative />,
  }));
  const create = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/instance/orgs", { body: { name: name.trim(), ...(owner ? { owner } : {}) } })),
    invalidates: [["instance", "orgs"], ["session"], ["me", "memberships"]],
    onSuccess: (o) => {
      toast.success(t("instance.orgCreated", "{name} created", { name: o.name }));
      onClose();
      void navigate({ to: "/settings/instance/orgs/$org", params: { org: o.id } });
    },
  });
  const reason = name.trim() ? undefined : t("instance.orgNameNeeded", "Give the organization a name");
  return (
    <Dialog
      busy={create.isPending}
      open
      onOpenChange={(o) => !o && onClose()}
      size="sm"
      title={t("instance.createOrg", "Create organization")}
      description={t("instance.createOrgHint", "Without an owner chosen you become its owner; with one, you stay outside it.")}
      onSubmit={() => !reason && create.mutate()}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel", "Cancel")}</Button>
          <Button
            variant="primary"
            disabled={!!reason}
            {...(reason ? { disabledReason: reason } : {})}
            loading={create.isPending}
            onClick={() => create.mutate()}
          >
            {t("instance.createOrg", "Create organization")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-16">
        <Input label={t("common.name", "Name")} value={name} onChange={(e) => setName(e.target.value)} maxLength={100} autoFocus />
        <Combobox
          label={t("instance.ownerOptional", "Owner (optional)")}
          placeholder={t("instance.searchUsers", "Search users")}
          options={options}
          loading={users.isFetching}
          onInputValueChange={setQuery}
          value={owner}
          onValueChange={(v: string | null) => setOwner(v)}
          emptyText={t("instance.noUsersMatch", "No users match")}
        />
      </div>
    </Dialog>
  );
}
