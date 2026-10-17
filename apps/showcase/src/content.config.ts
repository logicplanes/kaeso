import { defineCollection } from "astro:content";
import { changelogLoader, changelogSchema } from "./lib/changelog";

export const collections = {
  changelog: defineCollection({ loader: changelogLoader(new URL("../../../CHANGELOG.md", import.meta.url)), schema: changelogSchema }),
};
