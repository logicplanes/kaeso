import { useState, type ReactNode } from "react";
import {
  Badge,
  Banner,
  Button,
  ButtonGroup,
  Checkbox,
  CheckboxGroup,
  Combobox,
  Autocomplete,
  Dialog,
  Gauge,
  IconButton,
  Input,
  KeyDisplay,
  Menu,
  Meter,
  MetricRow,
  PillTabs,
  PortalContainer,
  Radio,
  RadioGroup,
  SearchInput,
  Segmented,
  Select,
  Slider,
  Sparkline,
  Stat,
  Switch,
  TagInput,
  ToastCapsule,
  Tooltip,
  TooltipProvider,
  icons,
  toast,
} from "@logicplanes/kaeso-ui";

import { keys, models } from "~/fixtures/kaeso";
import { DocsLocale, docsText, useDocsLang } from "~/lib/locale";
import { localizedPath } from "~/lib/languages";
import demos from "~/generated/demos.json";

function Cell({ slug, children }: { slug: keyof typeof demos; children: ReactNode }) {
  const title = demos[slug].title;
  const { lang } = useDocsLang();
  return (
    <section aria-label={title} className="group/cell flex min-h-240 min-w-0 flex-col bg-panel p-24 md:min-h-320">
      <a
        href={localizedPath(lang, `/components/${slug}`)}
        className="-mx-6 inline-flex h-24 items-center gap-6 self-start rounded-pill px-6 text-body font-regular text-fg-secondary transition-control hover:text-fg focus-ring"
      >
        {title}
        <span aria-hidden="true" className="text-fg-tertiary opacity-0 transition-control group-hover/cell:opacity-100">
          →
        </span>
      </a>
      <div className="flex min-w-0 flex-1 items-center justify-center py-24">
        <div className="flex w-full max-w-280 flex-col items-center gap-12">{children}</div>
      </div>
    </section>
  );
}

const modelOptions = models.map((m) => ({ value: m.model, label: m.model, description: m.name }));

function CreateKey({ t }: { t: (s: string) => string }) {
  const [open, setOpen] = useState(false);
  const create = () => {
    setOpen(false);
    toast.success(t("Key created"));
  };
  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      trigger={<Button variant="primary">{t("Create key")}</Button>}
      title={t("Create key")}
      description={t("Applications use this key to call the gateway.")}
      onSubmit={create}
      footer={
        <>
          <Dialog.Close>
            <Button>{t("Cancel")}</Button>
          </Dialog.Close>
          <Button variant="primary" onClick={create} shortcut={["mod", "enter"]}>
            {t("Create key")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-20">
        <Input label={t("Name")} placeholder="checkout-web" autoFocus />
        <Select
          label={t("Expires")}
          options={[
            { value: "never", label: t("Never") },
            { value: "90", label: t("In 90 days") },
          ]}
          defaultValue="90"
          fullWidth
        />
      </div>
    </Dialog>
  );
}

export default function ComponentGrid() {
  return (
    <DocsLocale>
      <PortalContainer>
        <TooltipProvider>
          <Cells />
        </TooltipProvider>
      </PortalContainer>
    </DocsLocale>
  );
}

function Cells() {
  const { lang } = useDocsLang();
  const t = (s: string) => docsText(lang, s);
  const email = (v: string) => (/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(v) ? true : t("“{value}” is not an email address.").replace("{value}", v));
  const retention = [
    { value: "7", label: t("7 days") },
    { value: "30", label: t("30 days") },
    { value: "90", label: t("90 days") },
  ];
  return (
    <>
      <div className="grid grid-cols-1 gap-1 bg-line hairline-b-strong md:grid-cols-2 xl:grid-cols-3 3xl:grid-cols-4">
        <Cell slug="button">
          <Button leadingIcon={icons.add}>{t("Create key")}</Button>
          <Button variant="primary" leadingIcon={icons.add}>
            {t("Create key")}
          </Button>
          <Button variant="ghost">{t("Cancel")}</Button>
        </Cell>
        <Cell slug="button-group">
          <ButtonGroup aria-label={t("Export")}>
            <ButtonGroup.Item>CSV</ButtonGroup.Item>
            <ButtonGroup.Item>JSON</ButtonGroup.Item>
            <ButtonGroup.Item>NDJSON</ButtonGroup.Item>
          </ButtonGroup>
        </Cell>
        <Cell slug="input">
          <Input aria-label={t("Key name")} placeholder="checkout-web" />
          <Input aria-label={t("Monthly budget")} defaultValue="500.00" suffix="USD" />
        </Cell>
        <Cell slug="tag-input">
          <div className="w-full">
            <TagInput
              className="w-full"
              aria-label={t("Invite members")}
              placeholder="name@example.com"
              defaultValue={["lin@northwind.example", "ada@northwind.example"]}
              validate={email}
            />
          </div>
        </Cell>

        <Cell slug="select">
          <div className="w-full">
            <Select aria-label={t("Log retention")} options={retention} defaultValue="30" fullWidth />
          </div>
        </Cell>
        <Cell slug="search-input">
          <div className="flex w-full items-center gap-8">
            <div className="min-w-0 flex-1">
              <SearchInput aria-label={t("Search logs")} placeholder={t("Search logs")} />
            </div>
            <IconButton icon={icons.filter} aria-label={t("Filter")} variant="secondary" />
          </div>
        </Cell>
        <Cell slug="autocomplete">
          <div className="w-full">
            <Autocomplete
              className="w-full"
              aria-label={t("Endpoint")}
              leadingIcon={icons.route}
              suggestions={["/v1/responses", "/v1/chat/completions", "/v1/messages", "/v1/embeddings"]}
              placeholder="/v1/…"
            />
          </div>
        </Cell>
        <Cell slug="combobox">
          <div className="w-full">
            <Combobox aria-label={t("Model")} options={modelOptions} placeholder={t("Search models")} emptyText={t("No models match")} />
          </div>
        </Cell>

        <Cell slug="switch">
          <div className="flex flex-col gap-16">
            <Switch label={t("Send budget alerts")} defaultChecked />
            <Switch label={t("Log request content")} />
          </div>
        </Cell>
        <Cell slug="dialog">
          <CreateKey t={t} />
        </Cell>
        <Cell slug="tooltip">
          <div className="flex items-center gap-8">
            <Tooltip content={t("Go to request logs")} shortcut={[["g"], ["l"]]}>
              <Button>{t("Request logs")}</Button>
            </Tooltip>
            <IconButton icon={icons.filter} aria-label={t("Filter")} variant="secondary" />
          </div>
        </Cell>
        <Cell slug="menu">
          <Menu>
            <Menu.Trigger>
              <Button trailingIcon={icons.chevronDown}>{t("Key actions")}</Button>
            </Menu.Trigger>
            <Menu.Content>
              <Menu.Item icon={icons.edit}>{t("Rename")}</Menu.Item>
              <Menu.Item icon={icons.copy}>{t("Copy key id")}</Menu.Item>
              <Menu.Item icon={icons.limit}>{t("Edit limits")}</Menu.Item>
              <Menu.Separator />
              <Menu.Item icon={icons.delete} variant="danger">
                {t("Archive key")}
              </Menu.Item>
            </Menu.Content>
          </Menu>
        </Cell>

        <Cell slug="segmented">
          <Segmented
            aria-label={t("Period")}
            defaultValue="7d"
            options={[
              { value: "24h", label: "24h" },
              { value: "7d", label: "7d" },
              { value: "30d", label: "30d" },
              { value: "90d", label: "90d" },
            ]}
          />
        </Cell>
        <Cell slug="tabs">
          <PillTabs defaultValue="all">
            <PillTabs.List aria-label={t("Keys")}>
              <PillTabs.Tab value="all" count={6}>
                {t("All")}
              </PillTabs.Tab>
              <PillTabs.Tab value="active" count={3}>
                {t("Active")}
              </PillTabs.Tab>
              <PillTabs.Tab value="disabled">{t("Disabled")}</PillTabs.Tab>
            </PillTabs.List>
          </PillTabs>
        </Cell>
        <Cell slug="checkbox">
          <CheckboxGroup defaultValue={["text", "image"]} aria-label={t("Input types")}>
            <Checkbox value="text" label={t("Text")} />
            <Checkbox value="image" label={t("Images")} />
            <Checkbox value="pdf" label="PDF" />
          </CheckboxGroup>
        </Cell>
        <Cell slug="radio">
          <RadioGroup defaultValue="30" aria-label={t("Retention")}>
            <Radio value="7" label={t("7 days")} />
            <Radio value="30" label={t("30 days")} />
            <Radio value="90" label={t("90 days")} />
          </RadioGroup>
        </Cell>

        <Cell slug="slider">
          <div className="w-full">
            <Slider aria-label={t("Alert threshold")} defaultValue={80} formatValue={(v) => `${v}%`} />
          </div>
        </Cell>
        <Cell slug="meter">
          <div className="w-full">
            <Meter label={t("Monthly budget")} value={0.642} valueLabel={t("$32.10 of $50.00")} thresholds={[0.8]} caption={t("Resets in 12 days")} />
          </div>
        </Cell>
        <Cell slug="gauge">
          <Gauge aria-label={t("Monthly budget used")} value={0.64} label={t("of $500.00 budget")} valueLabel={t("$320.00")} />
        </Cell>
        <Cell slug="stat">
          <div className="flex w-full flex-col gap-16">
            <MetricRow>
              <Stat value={t("12,480")} unit={t("requests")} />
              <Stat value={t("$184.20")} unit={t("spend")} />
            </MetricRow>
            <Sparkline fluid aria-label={t("Requests over 14 days, rising")} values={[12, 18, 15, 22, 28, 26, 34, 31, 36, 40, 38, 44, 47, 52]} />
          </div>
        </Cell>

        <Cell slug="badge">
          <div className="flex flex-wrap items-center justify-center gap-8">
            <Badge>Inference</Badge>
            <Badge tone="success">{t("Active")}</Badge>
            <Badge tone="warning">{t("Expiring")}</Badge>
            <Badge tone="danger">{t("Failed")}</Badge>
          </div>
        </Cell>
        <Cell slug="banner">
          <div className="w-full">
            <Banner tone="warning">{t("This key expires in 5 days.")}</Banner>
          </div>
        </Cell>
        <Cell slug="toast">
          <ToastCapsule title={t("Key disabled")} action={t("Undo")} />
          <ToastCapsule tone="success" title={t("Provider saved")} />
        </Cell>
        <Cell slug="key-display">
          <div className="flex flex-col gap-12">
            {keys.slice(0, 2).map((k) => (
              <KeyDisplay key={k.id} redacted={k.redacted} state={k.state} access={k.access} />
            ))}
          </div>
        </Cell>
      </div>
    </>
  );
}
