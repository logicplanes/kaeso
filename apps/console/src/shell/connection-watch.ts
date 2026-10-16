import { useEffect } from "react";

let watching = 0;

export const connectionWatched = () => watching > 0;

export function useWatchConnection(): void {
  useEffect(() => {
    watching++;
    return () => {
      watching--;
    };
  }, []);
}
