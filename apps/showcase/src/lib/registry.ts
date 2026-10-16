import registryJson from "../../../../packages/ui/ai/component-registry.json";

export interface RegistryProp {
  name: string;
  type: string;
  required: boolean;
  default: string | null;
  description: string;
  deprecated?: boolean;
}

export interface RegistryComponent {
  name: string;
  description: string;
  props: RegistryProp[];
  file: string;
}

export interface RegistryVariants {
  axes: Record<string, Record<string, string>>;
  defaults: Record<string, string>;
}

export interface RegistryEntry {
  entry: string;
  kind: string;
  import: string;
  components: RegistryComponent[];
  variants: Record<string, RegistryVariants>;
}

export interface ComponentRegistry {
  package: string;
  version: string;
  components: RegistryEntry[];
}

export const registry: ComponentRegistry = registryJson;
