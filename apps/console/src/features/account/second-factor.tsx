import { Field, Input, Link, OTPInput, Text } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";

export interface SecondFactorValue {
  recovery: boolean;
  code: string;
}

export const NO_SECOND_FACTOR: SecondFactorValue = { recovery: false, code: "" };

export function secondFactorBody(v: SecondFactorValue): { code: string } | { recovery_code: string } {
  return v.recovery ? { recovery_code: v.code.trim() } : { code: v.code };
}

export function secondFactorReady(v: SecondFactorValue): boolean {
  return v.recovery ? v.code.trim().length > 0 : v.code.length === 6;
}

export function SecondFactorField({
  value,
  onChange,
  error,
}: {
  value: SecondFactorValue;
  onChange: (value: SecondFactorValue) => void;
  error?: string | undefined;
}) {
  const toggle = (
    <Text variant="small" tone="secondary">
      <Link href="#" onClick={(e) => (e.preventDefault(), onChange({ recovery: !value.recovery, code: "" }))}>
        {value.recovery ? t("account.useAppCode", "Use the authenticator app") : t("account.useRecoveryCode", "Use a recovery code")}
      </Link>
    </Text>
  );
  if (value.recovery) {
    return (
      <div className="flex flex-col gap-8">
        <Input
          label={t("account.recoveryCode", "Recovery code")}
          value={value.code}
          onChange={(e) => onChange({ recovery: true, code: e.target.value })}
          autoComplete="one-time-code"
          spellCheck={false}
          error={error}
        />
        {toggle}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-8">
      <Field label={t("account.appCode", "Code from the app")} error={error}>
        <OTPInput aria-label={t("account.appCode", "Code from the app")} value={value.code} onValueChange={(code) => onChange({ recovery: false, code })} />
      </Field>
      {toggle}
    </div>
  );
}
