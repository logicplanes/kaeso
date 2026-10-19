import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Dialog, DialogScope, InlineCopyText, Input, SettingsGroup, SettingsRow, SettingsSection, Text } from "@logicplanes/kaeso-ui";
import { toast } from "~/shell/toast";
import { formatDate } from "@logicplanes/kaeso-ui/format";
import { api, data } from "~/api/client";
import { memberViews } from "~/api/queries/invalidation";
import { meQuery } from "~/api/queries/me";
import { useApiMutation } from "~/auth/mutation";
import { LoadFailed } from "~/features/org/load-failed";
import { fieldError } from "~/api/field-error";
import { PictureControl, toPictureUpload } from "~/kaeso/picture";
import { SettingsLoading, SettingsPage } from "~/shell/settings-page";
import { t } from "~/i18n";

export function ProfilePage() {
  const me = useQuery(meQuery);
  const m = me.data;
  return (
    <SettingsPage title={t("account.profile", "Profile")} context={t("account.title", "Account")}>
      {me.isError ? (
        <LoadFailed title={t("account.profileFailed", "Your profile couldn't load")} />
      ) : !m ? (
        <SettingsLoading rows={5} />
      ) : (
        <>
          <SettingsSection title={t("account.title", "Account")}>
            <SettingsGroup>
              <PictureRow name={m.name} src={m.avatar_url} />
              <NameRow name={m.name} />
              <EmailRow email={m.email ?? ""} />
              <SettingsRow
                title={t("account.accountId", "Account id")}
                description={t("account.accountIdHint", "Used by instance administrators and in the audit log.")}
                control={<InlineCopyText value={m.id} label={t("account.copyAccountId", "Copy account id")} />}
              />
              <SettingsRow title={t("common.created", "Created")} control={<Text tone="secondary">{formatDate(m.created_at)}</Text>} />
            </SettingsGroup>
          </SettingsSection>
          <DeleteAccount email={m.email ?? ""} />
        </>
      )}
    </SettingsPage>
  );
}

function PictureRow({ name, src }: { name: string; src: string | null | undefined }) {
  const set = useApiMutation({
    mutationFn: async (file: File) => data(await api.PUT("/platform/v1/me/avatar", { body: await toPictureUpload(file) })),
    invalidates: [["me"], ["session"], memberViews, ["instance", "users"]],
    onSuccess: () => toast.success(t("account.pictureSaved", "Picture saved")),
  });
  const remove = useApiMutation({
    mutationFn: async () => data(await api.DELETE("/platform/v1/me/avatar")),
    invalidates: [["me"], ["session"], memberViews, ["instance", "users"]],
    onSuccess: () => toast(t("account.pictureRemoved", "Picture removed")),
  });
  return (
    <SettingsRow
      title={t("account.picture", "Picture")}
      description={t("account.pictureHint", "A square picture of you, shown with your name.")}
      control={<PictureControl name={name} src={src} onPick={(f) => set.mutate(f)} onRemove={() => remove.mutate()} busy={set.isPending || remove.isPending} />}
    />
  );
}

function NameRow({ name }: { name: string }) {
  const [draft, setDraft] = useState(name);
  const save = useApiMutation({
    mutationFn: async (next: string) => data(await api.PATCH("/platform/v1/me", { body: { name: next } })),
    invalidates: [["me"], ["session"]],
    onSuccess: () => toast.success(t("account.nameSaved", "Name saved")),
  });
  const ready = draft.trim().length > 0 && draft.trim() !== name;
  return (
    <SettingsRow
      title={t("common.name", "Name")}
      description={t("account.nameHint", "Shown to the members of your organizations.")}
      control={
        <div className="flex min-w-0 items-center gap-8">
          <div className="w-240 min-w-0">
            <Input
              aria-label={t("common.name", "Name")}
              size="sm"
              value={draft}
              maxLength={100}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && ready && !save.isPending && save.mutate(draft.trim())}
            />
          </div>
          <Button size="sm" disabled={!ready} loading={save.isPending} onClick={() => save.mutate(draft.trim())}>
            {t("common.save", "Save")}
          </Button>
        </div>
      }
    />
  );
}

function EmailRow({ email }: { email: string }) {
  const [open, setOpen] = useState(false);
  const [sent, setSent] = useState<string | null>(null);
  return (
    <>
      <SettingsRow
        title={t("common.email", "Email")}
        description={sent ? t("account.emailPending", "{email} · link sent to {next}", { email, next: sent }) : email}
        control={
          <Button size="sm" onClick={() => setOpen(true)}>
            {t("account.changeEmail", "Change email")}
          </Button>
        }
      />
      <DialogScope open={open}>
        <ChangeEmailDialog
          open={open}
          onOpenChange={setOpen}
          onSent={(next) => {
            setSent(next);
            setOpen(false);
          }}
        />
      </DialogScope>
    </>
  );
}

function ChangeEmailDialog({ open, onOpenChange: setOpen, onSent }: { open: boolean; onOpenChange: (open: boolean) => void; onSent: (email: string) => void }) {
  const [next, setNext] = useState("");
  const change = useApiMutation({
    mutationFn: async () => data(await api.POST("/platform/v1/me/email", { body: { email: next.trim() } })),
    onSuccess: () => {
      toast.success(t("account.emailLinkSent", "Confirmation link sent; your old address was notified"));
      onSent(next.trim());
    },
  });
  return (
    <Dialog
      busy={change.isPending}
      open={open}
      onOpenChange={setOpen}
      size="sm"
      title={t("account.changeEmail", "Change email")}
      description={t("account.changeEmailHint", "We send a confirmation link to the new address; the change takes effect when you open it.")}
      onSubmit={() => next.trim() && change.mutate()}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("common.cancel", "Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" disabled={!next.trim()} loading={change.isPending} onClick={() => change.mutate()}>
            {t("account.sendConfirmation", "Send confirmation link")}
          </Button>
        </>
      }
    >
      <Input
        label={t("account.newEmail", "New email")}
        type="email"
        autoComplete="email"
        value={next}
        onChange={(e) => setNext(e.target.value)}
        autoFocus
        error={fieldError(change.error, "email")}
      />
    </Dialog>
  );
}

function DeleteAccount({ email }: { email: string }) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const remove = useApiMutation({
    mutationFn: async () => {
      await api.DELETE("/platform/v1/me");
    },
    onSuccess: () => {
      queryClient.clear();
      window.location.assign("/sign-in");
    },
    onError: () => setOpen(false),
  });
  return (
    <SettingsSection title={t("account.deleteAccount", "Delete account")}>
      <SettingsGroup>
        <SettingsRow
          danger
          title={t("account.deleteYourAccount", "Delete your account")}
          description={t("account.deleteHint", "You leave every organization; this can't be undone.")}
          control={
            <Button variant="danger-secondary" size="sm" onClick={() => setOpen(true)}>
              {t("account.deleteAccount", "Delete account")}
            </Button>
          }
        />
      </SettingsGroup>
      <AlertDialog
        open={open}
        onOpenChange={setOpen}
        tone="danger"
        title={t("account.deleteConfirm", "Delete your account?")}
        description={t("account.deleteConfirmHint", 'Your keys are archived and you appear as "Deleted user" in history.')}
        confirmLabel={t("account.deleteAccount", "Delete account")}
        confirmText={email}
        loading={remove.isPending}
        onConfirm={() => remove.mutateAsync()}
      />
    </SettingsSection>
  );
}
