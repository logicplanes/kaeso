export const alertMarks = (limit: { period: string; alert_thresholds: number[] }): number[] =>
  limit.period === "minute" ? [] : limit.alert_thresholds.map((percent) => percent / 100);
