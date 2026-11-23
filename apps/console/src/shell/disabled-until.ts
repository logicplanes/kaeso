/**
 * A button that waits for something: while `reason` says what's missing it stays focusable and says so on hover and to
 * screen readers; while the form is busy it is simply disabled.
 */
export const disabledUntil = (reason: string | undefined, busy = false) => ({
  disabled: !!reason || busy,
  ...(reason && !busy ? { disabledReason: reason } : {}),
});
