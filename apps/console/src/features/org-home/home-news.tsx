import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import {
  Announcement,
  Badge,
  Button,
  Dashboard,
  DashboardCell,
  Dialog,
  ModelCard,
  ModelLabel,
  ProviderBadge,
  RankList,
  ReleaseArt,
  Text,
  isReleaseArtSpec,
  type RankItem,
} from "@logicplanes/kaeso-ui";
import { formatContext, formatDate, formatPrice } from "@logicplanes/kaeso-ui/format";
import type { SearchHit } from "~/api/generated/types";
import { newestModelsQuery } from "~/api/queries/models";
import { whatsNewQuery } from "~/api/queries/whats-new";
import { t } from "~/i18n";
import { useProviders } from "~/kaeso/providers";
import { projectPath } from "~/shell/pages";
import { showWhatsNew } from "~/shell/whats-new-request";

const NEW_FOR_DAYS = 45;
const firstSentence = (notes: string) =>
  notes
    .split("\n")
    .filter((l) => !l.trimStart().startsWith("#"))
    .map((l) =>
      l
        .replace(/^[>*\-\s]+/, "")
        .replace(/\*\*|__|`/g, "")
        .trim(),
    )
    .find((l) => l.length > 0) ?? "";

function useReleases() {
  const news = useQuery(whatsNewQuery);
  const all = news.data ? [...news.data.releases, news.data.running] : [];
  const unique = all.filter((r, i) => all.findIndex((x) => x.version === r.version) === i);
  return { loading: news.isPending, releases: unique.sort((a, b) => (b.published_at ?? "").localeCompare(a.published_at ?? "")), running: news.data?.running };
}

export function ReleaseBanner() {
  const { releases, running } = useReleases();
  const latest = releases[0];
  if (!latest) return null;
  const isNew = latest.version !== running?.version;
  return (
    <Dashboard aria-label={t("kaeso.home.news", "News")}>
      <Announcement
        className="col-span-full"
        eyebrow={isNew ? t("kaeso.home.newRelease", "New release") : t("kaeso.home.running", "Running now")}
        title={t("kaeso.home.releaseTitle", "Kaeso {version}", { version: latest.version })}
        description={firstSentence(latest.notes) || undefined}
        actions={
          <Button size="sm" onClick={() => showWhatsNew()}>
            {t("kaeso.home.readWhatsNew", "Read what's new")}
          </Button>
        }
        art={
          <ReleaseArt
            seed={latest.version}
            art={isReleaseArtSpec(latest.art) ? latest.art : undefined}
            label={t("shell.whatsNew.artLabel", "Kaeso {version}", { version: latest.version })}
            shape="wide"
            flush
          />
        }
      />
    </Dashboard>
  );
}

function NewModelCards({ org, project, className }: { org: string; project: string; className: string }) {
  const providers = useProviders(org);
  const models = useQuery({ ...newestModelsQuery(org, project), retry: false });
  const base = projectPath(org, project);
  const [recent] = useState(() => Date.now() - NEW_FOR_DAYS * 86_400_000);
  if (models.isError) return null;
  if (!models.data)
    return (
      <>
        {[0, 1].map((i) => (
          <DashboardCell key={i} className={className}>
            <RankList aria-label={t("kaeso.home.newModels", "New models")} items={[]} loading rows={3} />
          </DashboardCell>
        ))}
      </>
    );
  return (
    <>
      {models.data.slice(0, 2).map((m) => {
        const fresh = m.release_date && Date.parse(m.release_date) >= recent;
        return (
          <ModelCard
            key={`${m.provider}/${m.model_id}`}
            className={className}
            mark={<ProviderBadge provider={m.provider} name={providers.name(m.provider)} size="lg" />}
            name={m.name || m.model_id}
            model={m.model_id}
            {...(fresh ? { badge: <Badge tone="ink">{t("kaeso.home.new", "New")}</Badge> } : {})}
            detail={m.release_date ? t("kaeso.home.released", "Released {date}", { date: formatDate(m.release_date) }) : providers.name(m.provider)}
            facts={[
              ...(m.context_limit ? [{ label: t("kaeso.home.context", "Context"), value: formatContext(m.context_limit) }] : []),
              ...(m.input_price && m.output_price
                ? [
                    {
                      label: t("kaeso.home.price", "Per 1M tokens"),
                      value: t("kaeso.home.inOutPrice", "{input} in, {output} out", { input: formatPrice(m.input_price), output: formatPrice(m.output_price) }),
                    },
                  ]
                : []),
            ]}
            action={
              <Button size="sm" href={`${base}/models`}>
                {m.catalogue_model_id ? t("kaeso.home.openModel", "Open in models") : t("kaeso.home.addModel", "Add to this project")}
              </Button>
            }
          />
        );
      })}
      <MoreModels org={org} project={project} models={models.data} className={className} />
    </>
  );
}

const MORE_ROWS = 4;
const LIST_DAYS = 90;

function MoreModels({ org, project, models, className }: { org: string; project: string; models: SearchHit[]; className: string }) {
  const [open, setOpen] = useState(false);
  const [since] = useState(() => Date.now() - LIST_DAYS * 86_400_000);
  const providers = useProviders(org);
  const navigate = useNavigate();
  const base = projectPath(org, project);
  const recent = models.filter((m) => m.release_date && Date.parse(m.release_date) >= since);
  const list = recent.length >= MORE_ROWS + 2 ? recent : models.slice(0, 30);
  const row = (m: SearchHit): RankItem => ({
    id: `${m.provider}/${m.model_id}`,
    label: (
      <ModelLabel
        className="min-w-0"
        provider={m.provider}
        providerName={providers.name(m.provider)}
        model={m.model_id}
        {...(m.name && m.name !== m.model_id ? { name: m.name } : {})}
      />
    ),
    secondary: m.release_date ? formatDate(m.release_date) : undefined,
  });
  return (
    <DashboardCell
      className={className}
      title={t("kaeso.home.moreModels", "More new models")}
      actions={
        <Button size="sm" variant="ghost" onClick={() => setOpen(true)}>
          {t("kaeso.home.seeAll", "See all {count}", { count: list.length })}
        </Button>
      }
    >
      <RankList aria-label={t("kaeso.home.moreModels", "More new models")} items={list.slice(2, 2 + MORE_ROWS).map(row)} onOpen={() => setOpen(true)} />
      <Dialog
        open={open}
        onOpenChange={setOpen}
        size="lg"
        title={t("kaeso.home.newModels", "New models")}
        description={t("kaeso.home.newModelsHint", "Newest first, from the model catalogue Kaeso keeps up to date.")}
      >
        <RankList
          aria-label={t("kaeso.home.newModels", "New models")}
          items={list.map(row)}
          onOpen={(id) => {
            setOpen(false);
            void navigate({ href: `${base}/models?provider=${encodeURIComponent(id.slice(0, id.indexOf("/")))}` });
          }}
        />
      </Dialog>
    </DashboardCell>
  );
}

function SectionTitles({ titles }: { titles: { id: string; title: string; span: string }[] }) {
  return (
    <div className="mt-48 mb-12 grid grid-cols-1 gap-1 lg:grid-cols-3">
      {titles.map((s) => (
        <Text key={s.id} as="h2" variant="title-sm" className={s.span}>
          {s.title}
        </Text>
      ))}
    </div>
  );
}

export function NewModels({ org, project }: { org: string; project: string }) {
  return (
    <>
      <SectionTitles titles={[{ id: "models", title: t("kaeso.home.newModels", "New models"), span: "lg:col-span-3" }]} />
      <Dashboard aria-label={t("kaeso.home.newModels", "New models")} columns={3}>
        <NewModelCards org={org} project={project} className="lg:col-span-1" />
      </Dashboard>
    </>
  );
}
