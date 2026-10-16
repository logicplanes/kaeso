type Refusal = { message: string; param?: string | null | undefined };

export const fieldError = (error: Refusal | null | undefined, param: string): string | undefined => (error?.param === param ? error.message : undefined);
