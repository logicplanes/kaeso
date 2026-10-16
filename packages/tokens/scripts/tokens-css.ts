const themeSelectors = {
  light: ':root, [data-theme="light"]',
  systemDark: ':root:not([data-theme="light"])',
  dark: '[data-theme="dark"]',
} as const;

export function tokensCss({ fixed, light, dark }: { fixed: readonly string[]; light: readonly string[]; dark: readonly string[] }): string {
  return `
${themeSelectors.light} {
${fixed.join("\n")}
${light.join("\n")}
}

@media (prefers-color-scheme: dark) {
  ${themeSelectors.systemDark} {
${dark.map((l) => `  ${l}`).join("\n")}
  }
}

${themeSelectors.dark} {
${dark.join("\n")}
}

@media (prefers-contrast: more) {
  ${themeSelectors.systemDark}, [data-theme] {
    --k-line: var(--k-line-strong);
    --k-line-subtle: var(--k-line-strong);
    --k-text-secondary: var(--k-text);
    --k-text-tertiary: var(--k-text);
  }
}
`;
}
