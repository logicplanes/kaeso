import { Avatar, KeyValue, StepDialog } from "@logicplanes/kaeso-ui";
import { api, data } from "~/api/client";
import { projectListKey, projectViewKey } from "~/api/queries/projects";
import type { ProjectView } from "~/api/generated/types";
import { useApiMutation } from "~/auth/mutation";
import { toast } from "~/shell/toast";
import { t } from "~/i18n";

function Named({ project }: { project: ProjectView }) {
  return (
    <span className="flex min-w-0 items-center gap-8">
      <Avatar name={project.name} {...(project.avatar_url ? { src: project.avatar_url } : {})} kind="org" size={20} decorative />
      <span className="truncate">{project.name}</span>
    </span>
  );
}

export function MakeDefaultDialog({
  org,
  project,
  current,
  onClose,
}: {
  org: string;
  project: ProjectView;
  current: ProjectView | undefined;
  onClose: () => void;
}) {
  const promote = useApiMutation({
    mutationFn: async () => data(await api.PUT("/platform/v1/orgs/{org}/default-project", { params: { path: { org } }, body: { project_id: project.id } })),
    invalidates: [
      projectListKey(org),
      projectViewKey(org, project.id),
      ...(current ? [projectViewKey(org, current.id)] : []),
      ["me", "memberships"],
      ["session"],
    ],
    onSuccess: (p) => {
      toast.success(t("orgHome.defaultMade", "{name} is the default project", { name: p.name }));
      onClose();
    },
  });
  return (
    <StepDialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t("orgHome.makeDefaultProject", "Make default project")}
      submitLabel={t("orgHome.makeDefault", "Make default")}
      submitting={promote.isPending}
      onSubmit={() => promote.mutate()}
      steps={[
        {
          id: "confirm",
          title: t("orgHome.makeDefaultQuestion", "Make {name} the default?", { name: project.name }),
          description: t("orgHome.makeDefaultHint", "Every member joins it, and people who join later start there."),
          content: (
            <KeyValue
              items={[
                ...(current ? [{ label: t("orgHome.defaultNow", "Default now"), value: <Named project={current} /> }] : []),
                { label: t("orgHome.newDefault", "New default"), value: <Named project={project} /> },
                ...(current
                  ? [
                      {
                        label: t("orgHome.afterwards", "Afterwards"),
                        value: t("orgHome.previousDefaultStays", "{name} stays a project; its members keep their places.", { name: current.name }),
                        wrap: true,
                      },
                    ]
                  : []),
              ]}
            />
          ),
        },
      ]}
    />
  );
}
