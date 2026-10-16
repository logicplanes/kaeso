import { RuleTester } from "eslint";
import { describe, it } from "vitest";
import kaeso from "./kaeso.js";

RuleTester.describe = describe;
RuleTester.it = it;
const tester = new RuleTester({ languageOptions: { ecmaVersion: 2024, sourceType: "module", parserOptions: { ecmaFeatures: { jsx: true } } } });

tester.run("layout-only", kaeso.rules["layout-only"], {
  valid: [
    { code: '<div className="flex flex-col gap-16 mt-24 w-full min-w-0" />' },
    { code: '<div className="grid grid-cols-2 lg:grid-cols-3 col-span-2 items-center justify-between" />' },
    { code: '<div className="relative overflow-hidden h-full min-h-0 text-center truncate z-10" />' },
    { code: '<div className={cn("flex", open && "hidden")} />' },
    { code: '<div className="w-160 -mt-8 max-w-640 gap-x-12" />' },
  ],
  invalid: [
    { code: '<div className="bg-panel" />', errors: 1 },
    { code: '<div className="p-16" />', errors: 1 },
    { code: '<div className="px-24 py-8" />', errors: 2 },
    { code: '<div className="h-36" />', errors: 1 },
    { code: '<div className="text-small text-fg-secondary" />', errors: 2 },
    { code: '<div className="rounded-frame shadow-ring border" />', errors: 3 },
    { code: '<div className="font-medium md:hover:bg-fill-1" />', errors: 2 },
    { code: '<div className={cn("flex", active && "text-fg")} />', errors: 1 },
    { code: "<div style={{ color: 'red' }} />", errors: 1 },
    { code: '<div className="w-176 mt-25" />', errors: 2 },
  ],
});

tester.run("no-color", kaeso.rules["no-color"], {
  valid: [{ code: 'const a = "#add-filter";' }, { code: 'const url = "/invite#token=abc";' }, { code: "const id = 'req_01ABC';" }],
  invalid: [
    { code: 'const c = "#ffffff";', errors: 1 },
    { code: "const c = `rgb(0 0 0)`;", errors: 1 },
    { code: 'const c = "#0a0a0a";', errors: 1 },
  ],
});

tester.run("no-literal-text", kaeso.rules["no-literal-text"], {
  valid: [
    { code: '<Button>{t("keys.create", "Create key")}</Button>' },
    { code: '<Input aria-label={t("signIn.email", "Email")} placeholder={t("signIn.emailAddress", "Email address")} />' },
    { code: 'toast.success(t("keys.revoked", "{name} revoked", { name }));' },
    { code: "<Text>{name} · {formatDate(at)}</Text>" },
    { code: "<Badge>OpenAI</Badge>" },
    { code: '<Code>{"{"}</Code>' },
    { code: 'const options = [{ value: "day", label: t("format.day", "Day") }];' },
    { code: '<div className="flex" data-state="open" />' },
  ],
  invalid: [
    { code: "<Button>Create key</Button>", errors: 1 },
    { code: '<Input placeholder="Email address" />', errors: 1 },
    { code: '<IconButton aria-label="Close" />', errors: 1 },
    { code: '<Row title={done ? "Saved" : "Not saved"} />', errors: 2 },
    { code: "<Text>{`${count} keys`}</Text>", errors: 1 },
    { code: 'toast.error("Something went wrong");', errors: 1 },
    { code: 'const columns = [{ id: "name", label: "Name" }];', errors: 1 },
    { code: 'usePageTitle("API keys");', errors: 1 },
  ],
});
