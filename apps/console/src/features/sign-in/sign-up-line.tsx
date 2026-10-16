import { useQuery } from "@tanstack/react-query";
import { Link, Text } from "@logicplanes/kaeso-ui";
import { signInMethodsQuery } from "~/auth/sign-in-api";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";

export function SignUpLine() {
  const install = useQuery(signInMethodsQuery);
  if (!install.data?.sign_up) return null;
  return (
    <Text variant="small" tone="secondary" className="text-center">
      {tRich("signIn.newHere", "New here? {link}", { link: <Link href="/sign-up">{t("signIn.createAccount", "Create an account")}</Link> })}
    </Text>
  );
}
