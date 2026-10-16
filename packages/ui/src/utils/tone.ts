export type StatusTone = "success" | "warning" | "danger" | "info";

export const STATUS_TONE_TEXT: Record<StatusTone, string> = {
  success: "text-success-text",
  warning: "text-warning-text",
  danger: "text-danger-text",
  info: "text-info-text",
};
