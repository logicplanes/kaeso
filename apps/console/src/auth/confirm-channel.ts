export const CONFIRMED_PATH = "/signed-in-again";

// The popup reports back over a same-origin channel, not window.opener: the console's
// Cross-Origin-Opener-Policy, or the identity provider's, can sever the opener on the way.
const CHANNEL = "kaeso-sign-in-again";

export type Confirmed = { error: string | null };

export function announceConfirmed(error: string | null): void {
  if (typeof BroadcastChannel === "undefined") return;
  const channel = new BroadcastChannel(CHANNEL);
  channel.postMessage({ error });
  channel.close();
}

export function onConfirmed(listener: (confirmed: Confirmed) => void): () => void {
  if (typeof BroadcastChannel === "undefined") return () => undefined;
  const channel = new BroadcastChannel(CHANNEL);
  channel.onmessage = (event: MessageEvent<unknown>) => {
    const message = event.data;
    if (!message || typeof message !== "object" || !("error" in message)) return;
    listener({ error: typeof message.error === "string" ? message.error : null });
  };
  return () => channel.close();
}
