import { Badge, Banner, Collapsible, JsonView, Link, Pill, Separator, Surface, Text, type ContentBlock, type ConversationMessage } from "@logicplanes/kaeso-ui";
import { t } from "~/i18n";
import { tRich } from "~/i18n/rich";
import { Markdown, safeHref } from "@logicplanes/kaeso-ui/markdown";

const roleLabel = (role: ConversationMessage["role"]): string =>
  ({
    system: t("logs.role.system", "System"),
    developer: t("logs.role.developer", "Developer"),
    user: t("logs.role.user", "User"),
    assistant: t("logs.role.assistant", "Assistant"),
    tool: t("logs.role.tool", "Tool"),
  })[role];

const isJson = (s: string) => {
  const text = s.trim();
  if (!/^[[{]/.test(text)) return false;
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
};

const DATA_IMAGE = /^data:image\/[a-z0-9.+-]+;/i;

function webHref(src: string): string | null {
  const href = safeHref(src);
  return href && /^https?:/.test(href) ? href : null;
}

function ImageInput({ src, alt, mediaType }: { src: string | undefined; alt: string | undefined; mediaType: string | undefined }) {
  if (src && DATA_IMAGE.test(src))
    return (
      <Surface kind="well" className="w-120 overflow-hidden">
        <img src={src} alt={alt ?? t("logs.conversation.imageInput", "Image input")} className="block aspect-square w-120 object-cover" />
      </Surface>
    );
  const href = src ? webHref(src) : null;
  if (href)
    return (
      <Link href={href} external target="_blank" rel="noopener noreferrer nofollow" className="w-max max-w-full wrap-anywhere">
        {t("logs.conversation.imageFrom", "Image from {host}", { host: new URL(href).host })}
      </Link>
    );
  return (
    <Pill size="sm" className="w-max">
      {mediaType ? t("logs.conversation.imageType", "Image {type}", { type: mediaType }) : t("logs.conversation.image", "Image")}
    </Pill>
  );
}

function Blocks({ blocks }: { blocks: ContentBlock[] }) {
  return (
    <div className="flex min-w-0 flex-col gap-12">
      {blocks.map((b, i) => {
        switch (b.type) {
          case "text":
            return <Markdown key={i} text={b.text} />;
          case "tool_call":
            return (
              <div key={i} className="flex flex-col gap-8">
                <Text variant="small" weight="medium" tone="secondary" className="flex min-w-0 flex-wrap items-baseline gap-x-8">
                  {tRich("logs.conversation.called", "Called {tool}", {
                    tool: (
                      <Text as="span" variant="small" mono>
                        {b.name}
                      </Text>
                    ),
                  })}
                  {b.id && (
                    <Text as="span" variant="small" mono tone="tertiary">
                      {b.id}
                    </Text>
                  )}
                </Text>
                <JsonView value={b.arguments} defaultDepth={3} />
              </div>
            );
          case "tool_result":
            return (
              <div key={i} className="flex gap-16">
                <Separator orientation="vertical" />
                <div className="flex min-w-0 flex-1 flex-col gap-8">
                  <Text variant="small" weight="medium" tone="secondary" className="flex flex-wrap items-center gap-x-8">
                    {t("logs.conversation.result", "Result")}
                    {b.tool_call_id && (
                      <Text as="span" variant="small" mono tone="tertiary">
                        {b.tool_call_id}
                      </Text>
                    )}
                    {b.is_error && <Badge tone="danger">{t("logs.inspector.error", "Error")}</Badge>}
                  </Text>
                  {typeof b.content !== "string" ? (
                    <Blocks blocks={b.content} />
                  ) : isJson(b.content) ? (
                    <Text as="pre" variant="small" mono className="whitespace-pre-wrap wrap-anywhere">
                      {b.content}
                    </Text>
                  ) : (
                    <Markdown text={b.content} />
                  )}
                </div>
              </div>
            );
          case "reasoning":
            return (
              <Collapsible
                key={i}
                title={
                  <Text as="span" tone="secondary">
                    {t("logs.conversation.reasoning", "Reasoning")}
                  </Text>
                }
                hint={
                  b.redacted
                    ? t("logs.conversation.redacted", "Redacted by the provider")
                    : b.text
                      ? undefined
                      : t("logs.conversation.hidden", "Hidden by the display setting")
                }
              >
                {b.redacted ? (
                  <Text variant="small" tone="secondary">
                    {t("logs.conversation.redactedHint", "The provider redacted this reasoning; only its encrypted form was returned.")}
                  </Text>
                ) : b.text ? (
                  <Markdown text={b.text} size="small" tone="secondary" />
                ) : (
                  <Text variant="small" tone="secondary">
                    {t("logs.conversation.hiddenHint", "The model reasoned, but the request did not ask to show its reasoning.")}
                  </Text>
                )}
              </Collapsible>
            );
          case "image":
            return <ImageInput key={i} src={b.src} alt={b.alt} mediaType={b.media_type} />;
          case "document":
            return (
              <Pill key={i} size="sm" className="w-max">
                {b.name ?? t("logs.conversation.document", "Document")}
              </Pill>
            );
          case "refusal":
            return (
              <Banner key={i} tone="warning" title={t("logs.conversation.refused", "The model refused.")}>
                {b.text}
              </Banner>
            );
        }
      })}
    </div>
  );
}

export default function Conversation({ messages }: { messages: ConversationMessage[] }) {
  return (
    <div className="flex flex-col gap-24">
      {messages.map((m, i) => {
        const blocks = typeof m.content === "string" ? [{ type: "text" as const, text: m.content }] : m.content;
        return (
          <section key={i} aria-label={roleLabel(m.role)} className="flex flex-col gap-8">
            <Text as="span" variant="caption" tone={m.role === "assistant" ? "default" : "secondary"}>
              {roleLabel(m.role)}
            </Text>
            <Blocks blocks={blocks} />
          </section>
        );
      })}
    </div>
  );
}
