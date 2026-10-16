type Area = "local" | "session";

function storage(area: Area): Storage {
  return area === "local" ? window.localStorage : window.sessionStorage;
}

export function readStored(key: string, area: Area = "local"): string | null {
  try {
    return storage(area).getItem(key);
  } catch {
    return null;
  }
}

export function readStoredJson(key: string, area: Area = "local"): unknown {
  const raw = readStored(key, area);
  if (raw === null) return null;
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return null;
  }
}

export function writeStored(key: string, value: string | null, area: Area = "local"): boolean {
  try {
    if (value === null) storage(area).removeItem(key);
    else storage(area).setItem(key, value);
    return true;
  } catch {
    return false;
  }
}

export function takeStored(key: string, area: Area = "local"): string | null {
  const value = readStored(key, area);
  writeStored(key, null, area);
  return value;
}
