export const UPDATE_SERVICE = "https://update.kaeso.app";

export const releaseArtUrl = (version: string) => `${UPDATE_SERVICE}/art/${encodeURIComponent(version)}.svg`;
