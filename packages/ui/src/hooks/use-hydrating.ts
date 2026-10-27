import { useSyncExternalStore } from "react";

const noSubscription = () => () => undefined;
const notHydrating = () => false;
const isHydrating = () => true;

export const useHydrating = (): boolean => useSyncExternalStore(noSubscription, notHydrating, isHydrating);
