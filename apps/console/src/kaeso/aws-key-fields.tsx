import { Input } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export type AwsMethod = "aws_access_key" | "aws_role";

export type AwsKey = {
  accessKeyId: string;
  secretAccessKey: string;
  sessionToken: string;
  roleArn: string;
  externalId: string;
};

export const emptyAwsKey: AwsKey = { accessKeyId: "", secretAccessKey: "", sessionToken: "", roleArn: "", externalId: "" };

const ACCESS_KEY_ID = /^[A-Za-z0-9]{16,128}$/;
const ROLE_ARN = /^arn:aws[a-z-]*:iam::\d{12}:role\/\S+$/;
const EXTERNAL_ID = /^[\w+=,.@:/-]{2,1224}$/;

export function awsKeyBlocked(method: AwsMethod, key: AwsKey): string | undefined {
  if (method === "aws_access_key") {
    if (!ACCESS_KEY_ID.test(key.accessKeyId.trim())) return t("kaeso.providerKey.awsAccessKeyIdBlocked", "Give the access key ID");
    if (!key.secretAccessKey.trim() || /\s/.test(key.secretAccessKey.trim())) return t("kaeso.providerKey.awsSecretBlocked", "Paste the secret access key");
    if (/\s/.test(key.sessionToken.trim())) return t("kaeso.providerKey.awsTokenBlocked", "The session token can't contain spaces");
    return undefined;
  }
  if (key.roleArn.trim() && !ROLE_ARN.test(key.roleArn.trim())) return t("kaeso.providerKey.awsRoleBlocked", "Give a role ARN such as arn:aws:iam::…:role/…");
  if (key.externalId.trim() && !EXTERNAL_ID.test(key.externalId.trim())) return t("kaeso.providerKey.awsExternalIdBlocked", "An external ID has no spaces");
  return undefined;
}

/** The secret the Platform API takes for an AWS key: one JSON object. */
export function awsSecret(method: AwsMethod, key: AwsKey): string {
  if (method === "aws_access_key")
    return JSON.stringify({
      access_key_id: key.accessKeyId.trim(),
      secret_access_key: key.secretAccessKey.trim(),
      ...(key.sessionToken.trim() ? { session_token: key.sessionToken.trim() } : {}),
    });
  return JSON.stringify({
    ...(key.roleArn.trim() ? { role_arn: key.roleArn.trim() } : {}),
    ...(key.externalId.trim() ? { external_id: key.externalId.trim() } : {}),
  });
}

export function AwsKeyFields({ method, value, onValue }: { method: AwsMethod; value: AwsKey; onValue: (key: AwsKey) => void }) {
  const field = (name: keyof AwsKey) => ({
    value: value[name],
    onChange: (e: { target: { value: string } }) => onValue({ ...value, [name]: e.target.value }),
    autoComplete: "off",
    spellCheck: false,
  });
  if (method === "aws_access_key")
    return (
      <>
        <Input
          label={t("kaeso.providerKey.awsAccessKeyId", "Access key ID")}
          placeholder={t("kaeso.providerKey.awsAccessKeyIdPlaceholder", "AKIA…")}
          {...field("accessKeyId")}
          autoFocus
        />
        <Input secret label={t("kaeso.providerKey.awsSecretAccessKey", "Secret access key")} {...field("secretAccessKey")} />
        <Input
          secret
          label={t("kaeso.providerKey.awsSessionToken", "Session token")}
          description={t("kaeso.providerKey.awsSessionTokenHint", "Only for temporary credentials; they stop working when they expire.")}
          {...field("sessionToken")}
        />
      </>
    );
  return (
    <>
      <Input
        label={t("kaeso.providerKey.awsRoleArn", "Role to assume")}
        description={t("kaeso.providerKey.awsRoleArnHint", "Leave it empty to use the gateway's own role.")}
        placeholder={t("kaeso.providerKey.awsRoleArnExample", "arn:aws:iam::123456789012:role/kaeso-bedrock")}
        {...field("roleArn")}
        autoFocus
      />
      <Input
        label={t("kaeso.providerKey.awsExternalId", "External ID")}
        description={t("kaeso.providerKey.awsExternalIdHint", "Only when the role's trust policy asks for one.")}
        {...field("externalId")}
      />
    </>
  );
}
