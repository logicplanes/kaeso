let address: string | null = null;

export function handResetAddress(email: string): void {
  address = email;
}

export function takeResetAddress(): string {
  const taken = address ?? "";
  address = null;
  return taken;
}
