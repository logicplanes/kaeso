const pattern = /^KAESO_(.+?)_(DEFAULT_VARIANTS|VARIANTS)$/;

export function variantExport(name) {
  const match = pattern.exec(name);
  if (!match) return null;
  return { component: match[1], part: match[2] === "VARIANTS" ? "variants" : "defaults" };
}

export function variantProblems(variants) {
  return Object.entries(variants).flatMap(([component, v]) => {
    if (!v.variants) return [`${v.entry} exports KAESO_${component}_DEFAULT_VARIANTS without KAESO_${component}_VARIANTS`];
    if (!v.defaults) return [`${v.entry} exports KAESO_${component}_VARIANTS without KAESO_${component}_DEFAULT_VARIANTS`];
    return Object.entries(v.defaults)
      .filter(([axis, value]) => !Object.hasOwn(v.variants[axis] ?? {}, value))
      .map(([axis, value]) => `${v.entry}: KAESO_${component}_DEFAULT_VARIANTS sets ${axis} to ${value}, which KAESO_${component}_VARIANTS does not list`);
  });
}
