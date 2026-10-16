interface TokenCounts {
  input_tokens: number;
  output_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
}

export const inputTokens = (u: TokenCounts): number => u.input_tokens + u.cache_read_tokens + u.cache_write_tokens;

export const totalTokens = (u: TokenCounts): number => inputTokens(u) + u.output_tokens;
