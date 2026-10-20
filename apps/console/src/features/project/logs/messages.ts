import { useEffect, useState } from "react";

type GatewayErrors = typeof import("~/api/gateway-errors");
let gatewayErrors: GatewayErrors | undefined;
const loadGatewayErrors = () => import("~/api/gateway-errors").then((m) => (gatewayErrors = m));

export interface LogMessage {
  text: string;
  provider?: string | undefined;
}

const RESTATES_STATUS =
  /^(overloaded|internal server error|server error|not found|bad gateway|service unavailable|gateway timeout|timeout|too many requests|rate limit(ed| reached)?)\.?$/i;

export function logMessage(
  errors: GatewayErrors | undefined,
  message: string | null | undefined,
  provider: string | null | undefined,
  providerName: (id: string) => string,
): LogMessage | null {
  if (!message || RESTATES_STATUS.test(message.trim())) return null;
  if (!errors) return { text: message };
  const known = errors.knownMessage(message);
  if (known !== undefined) return { text: known };
  return provider ? { text: message, provider: providerName(provider) } : { text: message };
}

export function useGatewayErrors(): GatewayErrors | undefined {
  const [errors, setErrors] = useState(gatewayErrors);
  useEffect(() => {
    if (errors) return;
    let live = true;
    void loadGatewayErrors().then((m) => live && setErrors(m));
    return () => {
      live = false;
    };
  }, [errors]);
  return errors;
}
