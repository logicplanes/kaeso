import { useEffect } from "react";
import { useLocation } from "@tanstack/react-router";
import { takeFragment, useLinkVisit } from "./fragment";
import { showSsoError } from "./sso-errors";

export function SsoErrorHost() {
  const path = useLocation({ select: (l) => l.pathname });
  const visit = useLinkVisit();
  useEffect(() => {
    const code = takeFragment("sso_error");
    if (code) showSsoError(code);
  }, [path, visit]);
  return null;
}
