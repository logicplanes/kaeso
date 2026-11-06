import { Banner, Link } from "@logicplanes/kaeso-ui";
import { docsPage } from "~/format/docs";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

export function DemoNotice() {
  return (
    <Banner layout="full">
      {tRich("shell.demo.notice", "This is a read-only demo with sample data; {install} to connect your own providers.", {
        install: <Link href={docsPage("/self-hosting/installation")}>{t("shell.demo.install", "install Kaeso")}</Link>,
      })}
    </Banner>
  );
}
