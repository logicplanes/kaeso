import { Link, Select } from "@logicplanes/kaeso-ui";
import type { RegionsView } from "~/api/generated/types";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

/** The region a key of a provider whose keys each call one region, such as Amazon Bedrock, calls. */
export function RegionField({
  provider,
  regions,
  region,
  onRegion,
}: {
  provider: string;
  regions: RegionsView;
  region: string | null;
  onRegion: (region: string) => void;
}) {
  return (
    <Select
      fullWidth
      label={t("kaeso.providerKey.region", "Region")}
      description={tRich("kaeso.providerKey.regionHint", "Where {provider} runs the key's requests. {link}", {
        provider,
        link: (
          <Link href={regions.regions_page.url} external>
            {t("kaeso.providerKey.regionWhere", "Regions")}
          </Link>
        ),
      })}
      placeholder={t("kaeso.providerKey.regionPlaceholder", "Choose a region")}
      value={region}
      onValueChange={(v) => onRegion(v as string)}
      options={regions.regions.map((r) => ({ value: r, label: r }))}
    />
  );
}
