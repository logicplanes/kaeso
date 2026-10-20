import { forwardRef, lazy, Suspense, type ReactNode } from "react";
import { cn } from "../../utils/cn.js";
import { icons } from "../../icons/vocabulary.js";
import { Icon } from "../icon/icon.js";
import { Banner } from "../banner/banner.js";
import { Collapsible } from "../collapsible/collapsible.js";
import { JsonView } from "../code-block/json-view.js";
import { Badge } from "../badge/badge.js";
import { Link } from "../link/link.js";
import { safeHref } from "../../utils/safe-href.js";
import { useUiText, type UiText } from "../../i18n/context.js";

const LazyMarkdown = lazy(() => import("../markdown/markdown.js").then((m) => ({ default: m.Markdown })));

export const KAESO_CONVERSATION_VARIANTS = {
  density: {
    default: { classes: "gap-24", description: "24px between turns (default)" },
    compact: { classes: "gap-16", description: "16px, inside sheets" },
  },
} as const;
export const KAESO_CONVERSATION_DEFAULT_VARIANTS = { density: "default" } as const;

export type ContentBlock =
  | { type: "text"; text: string }
  | { type: "tool_call"; id?: string | undefined; name: string; arguments: unknown }
  | { type: "tool_result"; tool_call_id?: string | undefined; content: string | ContentBlock[]; is_error?: boolean | undefined }
  | { type: "reasoning"; text: string; redacted?: boolean | undefined }
  | { type: "image"; src?: string | undefined; alt?: string | undefined; media_type?: string | undefined }
  | { type: "document"; name?: string | undefined; media_type?: string | undefined }
  | { type: "refusal"; text: string };

export interface ConversationMessage {
  role: "system" | "user" | "assistant" | "tool" | "developer";
  content: string | ContentBlock[];
}

function roleNames(t: UiText): Record<ConversationMessage["role"], string> {
  return {
    system: t("conversationView.system", "System"),
    developer: t("conversationView.developer", "Developer"),
    user: t("conversationView.user", "User"),
    assistant: t("conversationView.assistant", "Assistant"),
    tool: t("conversationView.tool", "Tool"),
  };
}

const DATA_IMAGE = /^data:image\/[a-z0-9.+-]+;/i;

function webHref(src: string): string | null {
  const href = safeHref(src);
  return href && /^https?:/.test(href) ? href : null;
}

function isJson(s: string) {
  const t = s.trim();
  if (!/^[[{]/.test(t)) return false;
  try {
    JSON.parse(t);
    return true;
  } catch {
    return false;
  }
}

function Prose({ text, markdown, small }: { text: string; markdown: boolean; small?: boolean | undefined }) {
  const plain = (
    <p className={cn("max-w-prose whitespace-pre-wrap wrap-break-word text-pretty", small ? "text-small text-fg-secondary" : "text-body text-fg")}>{text}</p>
  );
  if (!markdown) return plain;
  return (
    <Suspense fallback={plain}>
      <LazyMarkdown text={text} {...(small ? { size: "small" as const, tone: "secondary" as const } : {})} />
    </Suspense>
  );
}

function Blocks({ blocks, markdown }: { blocks: ContentBlock[]; markdown: boolean }) {
  const t = useUiText();
  return (
    <div className="flex flex-col gap-12">
      {blocks.map((b, i) => {
        switch (b.type) {
          case "text":
            return <Prose key={i} text={b.text} markdown={markdown} />;
          case "tool_call":
            return (
              <div key={i} className="flex flex-col gap-8">
                <span className="inline-flex items-center gap-8 text-small font-medium text-fg-secondary">
                  <Icon icon={icons.tools} size="xs" />
                  {t("conversationView.called", "Called **{name}**", { name: b.name })
                    .split("**")
                    .map((part, j) =>
                      j % 2 ? (
                        <span key={j} className="mono text-fg">
                          {part}
                        </span>
                      ) : (
                        part
                      ),
                    )}
                  {b.id && <span className="mono text-fg-tertiary">{b.id}</span>}
                </span>
                <JsonView value={b.arguments} defaultDepth={3} />
              </div>
            );
          case "tool_result":
            return (
              <div key={i} className="flex flex-col gap-8 pl-16 hairline-l">
                <span className="inline-flex items-center gap-8 text-small font-medium text-fg-secondary">
                  {t("conversationView.result", "Result")}
                  {b.tool_call_id && <span className="mono text-fg-tertiary">{b.tool_call_id}</span>}
                  {b.is_error && <Badge tone="danger">{t("conversationView.error", "Error")}</Badge>}
                </span>
                {typeof b.content !== "string" ? (
                  <Blocks blocks={b.content} markdown={markdown} />
                ) : markdown && !isJson(b.content) ? (
                  <Prose text={b.content} markdown />
                ) : (
                  <pre className="mono text-small whitespace-pre-wrap wrap-anywhere text-fg">{b.content}</pre>
                )}
              </div>
            );
          case "reasoning":
            return (
              <Collapsible
                key={i}
                title={<span className="text-fg-secondary">{t("conversationView.reasoning", "Reasoning")}</span>}
                hint={b.redacted ? t("conversationView.redacted", "Redacted by the provider") : undefined}
              >
                {b.redacted ? (
                  <p className="max-w-prose text-small text-fg-secondary">
                    {t("conversationView.encrypted", "The provider returned this reasoning in encrypted form.")}
                  </p>
                ) : (
                  <Prose text={b.text} markdown={markdown} small />
                )}
              </Collapsible>
            );
          case "image": {
            if (b.src && DATA_IMAGE.test(b.src))
              return (
                <span key={i} className="block w-120 overflow-hidden rounded-well ring-over ring-over-line">
                  <img src={b.src} alt={b.alt ?? t("conversationView.imageInput", "Image input")} className="block h-120 w-120 object-cover" />
                </span>
              );
            const href = b.src ? webHref(b.src) : null;
            return href ? (
              <Link key={i} href={href} external rel="noopener noreferrer nofollow" className="w-max max-w-full wrap-anywhere">
                {t("conversationView.imageFrom", "Image from {host}", { host: new URL(href).host })}
              </Link>
            ) : (
              <span key={i} className="inline-flex h-32 w-max items-center gap-8 rounded-pill bg-fill-1 px-12 text-small text-fg-secondary">
                <Icon icon={icons.vision} size="xs" /> {t("conversationView.image", "Image")} {b.media_type && <span className="mono">{b.media_type}</span>}
              </span>
            );
          }
          case "document":
            return (
              <span key={i} className="inline-flex h-32 w-max items-center gap-8 rounded-pill bg-fill-1 px-12 text-small text-fg-secondary">
                <Icon icon={icons.pdf} size="xs" /> {b.name ?? t("conversationView.document", "Document")}
              </span>
            );
          case "refusal":
            return (
              <Banner key={i} tone="warning" title={t("conversationView.refused", "The model refused.")}>
                {b.text}
              </Banner>
            );
        }
      })}
    </div>
  );
}

export interface ConversationViewProps {
  messages: ConversationMessage[];
  density?: keyof typeof KAESO_CONVERSATION_VARIANTS.density | undefined;
  markdown?: boolean | undefined;
  className?: string | undefined;
  footer?: ReactNode | undefined;
}

export const ConversationView = forwardRef<HTMLDivElement, ConversationViewProps>(function ConversationView(
  { messages, density = "default", markdown = false, className, footer },
  ref,
) {
  const t = useUiText();
  const role = roleNames(t);
  return (
    <div ref={ref} className={cn("flex flex-col", KAESO_CONVERSATION_VARIANTS.density[density].classes, className)}>
      {messages.map((m, i) => (
        <section key={i} aria-label={role[m.role]} className="flex flex-col gap-8">
          <span className={cn("text-caption font-medium", m.role === "assistant" ? "text-fg" : "text-fg-secondary")}>{role[m.role]}</span>
          <div className={cn(m.role === "system" || m.role === "developer" ? "rounded-well bg-fill-1 px-16 py-12" : "")}>
            <Blocks blocks={typeof m.content === "string" ? [{ type: "text", text: m.content }] : m.content} markdown={markdown} />
          </div>
        </section>
      ))}
      {footer}
    </div>
  );
});
ConversationView.displayName = "ConversationView";
