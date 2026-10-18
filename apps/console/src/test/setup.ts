import "@testing-library/jest-dom/vitest";
import { cleanup, configure } from "@testing-library/react";
import { setFormatTimeZone } from "@logicplanes/kaeso-ui/format";
import { afterEach, beforeEach } from "vitest";
import { apply } from "~/i18n";
import { queryClient } from "~/query";
import { fetchStub, resetNetwork } from "./network";

configure({ asyncUtilTimeout: 3000 });

globalThis.fetch = fetchStub;

const defaults = queryClient.getDefaultOptions();
queryClient.setDefaultOptions({ ...defaults, queries: { ...defaults.queries, retryDelay: 0 } });

beforeEach(() => {
  resetNetwork();
});

afterEach(() => {
  cleanup();
  queryClient.clear();
  window.localStorage.clear();
  window.sessionStorage.clear();
  setFormatTimeZone(undefined);
  apply({ locale: "en", messages: {}, ui: {}, provider: undefined });
});
