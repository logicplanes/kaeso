import { Code, CopyButton, Field, Link, QRCode, Text } from "@logicplanes/kaeso-ui";
import { formatDuration } from "@logicplanes/kaeso-ui/format";
import type { DeviceSignIn } from "~/api/generated/types";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

export type DeviceState = "waiting" | "expired" | "ended";

export function DeviceCode({
  device,
  product,
  left,
  every,
  state,
}: {
  device: DeviceSignIn;
  product: string;
  left: number;
  every: number;
  state: DeviceState;
}) {
  const page = device.verification_uri_complete ?? device.verification_uri;
  const host = new URL(device.verification_uri).host;
  return (
    <div className="flex w-full flex-col items-start gap-20 sm:flex-row">
      <QRCode value={page} label={t("kaeso.providerKey.deviceQr", "QR code to open the {product} sign-in page", { product })} />
      <div className="flex min-w-0 flex-col gap-12">
        <Field label={t("kaeso.providerKey.deviceCode", "Code")}>
          <div className="flex items-center gap-8">
            <Code>{device.user_code}</Code>
            <CopyButton value={device.user_code} label={t("kaeso.providerKey.copyCode", "Copy code")} size="xs" />
          </div>
        </Field>
        <Text variant="small" tone="secondary">
          {tRich("kaeso.providerKey.deviceOpen", "Open {page} on any device, sign in to {product} and enter the code.", {
            page: (
              <Link href={page} external>
                {host}
              </Link>
            ),
            product,
          })}
        </Text>
        <Text variant="small" tone={state === "waiting" ? "secondary" : "danger"} role="status">
          {state === "expired"
            ? t("kaeso.providerKey.deviceExpired", "The code expired; get a new code to try again.")
            : state === "ended"
              ? t("kaeso.providerKey.deviceEnded", "This sign-in ended; get a new code to try again.")
              : t("kaeso.providerKey.deviceWaiting", "Waiting for your approval; the code expires in {time}.", { time: formatDuration(Math.max(left, 0)) })}
        </Text>
        {state === "waiting" && (
          <Text variant="small" tone="secondary">
            {t("kaeso.providerKey.deviceEvery", "Kaeso checks every {seconds} seconds.", { seconds: every })}
          </Text>
        )}
      </div>
    </div>
  );
}
