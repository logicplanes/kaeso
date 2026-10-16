import { useQueryClient } from "@tanstack/react-query";
import { createContext, Fragment, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { apply, load, resolve, storeChoice, storedChoice, type Loaded, type LocaleChoice } from "./index";

const Change = createContext<(choice: LocaleChoice) => Promise<void>>(async () => undefined);

export function I18nProvider({ initial, children }: { initial: Loaded; children: ReactNode }) {
  const [loaded, setLoaded] = useState(initial);
  const shown = useRef(initial.locale);
  const latest = useRef(0);
  const queryClient = useQueryClient();
  const change = useCallback(
    async (choice: LocaleChoice) => {
      storeChoice(choice);
      const ask = ++latest.current;
      const next = await load(resolve(choice));
      if (ask !== latest.current) return;
      apply(next);
      if (shown.current === next.locale) return;
      shown.current = next.locale;
      setLoaded(next);
      void queryClient.invalidateQueries();
    },
    [queryClient],
  );

  useEffect(() => {
    const follow = () => {
      if (storedChoice() === "auto") void change("auto");
    };
    window.addEventListener("languagechange", follow);
    return () => window.removeEventListener("languagechange", follow);
  }, [change]);

  return (
    <Change.Provider value={change}>
      {loaded.provider ? (
        <loaded.provider locale={loaded.locale} messages={loaded.ui}>
          <Fragment key={loaded.locale}>{children}</Fragment>
        </loaded.provider>
      ) : (
        <Fragment key={loaded.locale}>{children}</Fragment>
      )}
    </Change.Provider>
  );
}

export function useChangeLocale(): (choice: LocaleChoice) => Promise<void> {
  return useContext(Change);
}
