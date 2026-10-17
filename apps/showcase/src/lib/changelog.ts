import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Loader, LoaderContext } from "astro/loaders";
import { z } from "astro/zod";
import { history, type ChangelogEntry } from "../../../../scripts/changelog.mjs";

const topic = z.object({ name: z.string(), id: z.string(), html: z.string() });
const section = z.object({ name: z.string(), id: z.string(), html: z.string(), topics: z.array(topic) });
const entry = { id: z.string(), intro: z.string(), sections: z.array(section) };

export const changelogSchema = z.object({
  repository: z.string().nullable(),
  unreleased: z.object({ ...entry, since: z.string().nullable(), compare: z.string().nullable() }).nullable(),
  releases: z.array(
    z.object({
      ...entry,
      version: z.string(),
      date: z.string().nullable(),
      prerelease: z.boolean(),
      yanked: z.boolean(),
      latest: z.boolean(),
      previous: z.string().nullable(),
      url: z.string().nullable(),
      compare: z.string().nullable(),
    }),
  ),
});

export type RenderedChangelog = z.infer<typeof changelogSchema>;

export const CHANGELOG_GROUPS = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];

async function rendered(text: string, renderMarkdown: LoaderContext["renderMarkdown"]): Promise<RenderedChangelog> {
  const log = history(text);
  const html = async (markdown: string) => (markdown ? (await renderMarkdown(`${markdown}\n\n${log.definitions}`)).html : "");
  const each = <T, R>(items: T[], fn: (item: T) => Promise<R>) => Promise.all(items.map(fn));
  const part = async (e: ChangelogEntry) => ({
    id: e.id,
    intro: await html(e.intro),
    sections: await each(e.sections, async (s) => ({
      name: s.name,
      id: s.id,
      html: await html(s.markdown),
      topics: await each(s.topics, async (t) => ({ name: t.name, id: t.id, html: await html(t.markdown) })),
    })),
  });
  const { unreleased } = log;
  return {
    repository: log.repository,
    unreleased: unreleased && { ...(await part(unreleased)), since: unreleased.since, compare: unreleased.compare },
    releases: await each(log.releases, async (r) => ({
      ...(await part(r)),
      version: r.version,
      date: r.date,
      prerelease: r.prerelease,
      yanked: r.yanked,
      latest: r.latest,
      previous: r.previous,
      url: r.url,
      compare: r.compare,
    })),
  };
}

export function changelogLoader(file: URL): Loader {
  const path = fileURLToPath(file);
  return {
    name: "kaeso-changelog",
    load: async ({ store, parseData, renderMarkdown, generateDigest, watcher }) => {
      const sync = async () => {
        const text = readFileSync(path, "utf8");
        const data = await parseData({ id: "kaeso", data: await rendered(text, renderMarkdown) });
        store.clear();
        store.set({ id: "kaeso", data, digest: generateDigest(text) });
      };
      await sync();
      watcher?.add(path);
      watcher?.on("change", (changed) => {
        if (changed === path) void sync();
      });
    },
  };
}
