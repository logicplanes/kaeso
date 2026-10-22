import { useEffect } from "react";
import { useLocation } from "@tanstack/react-router";
import { takeFragment, useLinkVisit } from "./fragment";

export function SsoErrorHost() {
  const path = useLocation({ select: (l) => l.pathname });
  const visit = useLinkVisit();
  useEffect(() => {
    const code = takeFragment("sso_error");
    if (code) void import("./sso-errors").then((m) => m.showSsoError(code));
  }, [path, visit]);
  return null;
}
