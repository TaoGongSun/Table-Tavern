import { useEffect, useMemo, useRef } from "react";
import { renderStoryMarkdown } from "@desktop/shared/ui/story-markdown";
import { t } from "../../i18n";
import type { CharacterCardData } from "../cards/sample-card";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { quotaBlocksSending } from "../funnel/quota";
import type { ChatEntry } from "./chat-turn";
import { useChat } from "./useChat";

// 模型輸出與卡片文字一律走桌面版的安全渲染（DOMPurify），不直接塞 HTML（計畫 2.4）
function Message({ entry, name }: { entry: ChatEntry; name: string }) {
  const html = useMemo(() => renderStoryMarkdown(entry.text), [entry.text]);
  return (
    <article className={`message message-${entry.role}`} data-testid={`message-${entry.role}`}>
      {entry.role === "char" && <div className="message-name">{name}</div>}
      <div className="message-body" dangerouslySetInnerHTML={{ __html: html }} />
      {entry.interrupted && <div className="message-flag">{t("chatInterrupted")}</div>}
    </article>
  );
}

export function ChatView({
  card,
  session,
  onBack,
}: {
  card: CharacterCardData;
  session: OpenRouterSession;
  onBack: () => void;
}) {
  const chat = useChat(card, session);
  const endRef = useRef<HTMLDivElement>(null);
  const blocked = quotaBlocksSending(session.quota);
  const streamingHtml = useMemo(() => renderStoryMarkdown(chat.streaming), [chat.streaming]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [chat.entries, chat.streaming]);

  return (
    <section className="chat">
      <div className="chat-bar">
        <button type="button" className="ghost" onClick={onBack} disabled={chat.busy}>
          ← {t("chatBack")}
        </button>
        <h2>{card.name}</h2>
        <span className="chat-note">{t("memoryOnlyNote")}</span>
      </div>

      <div className="chat-log" aria-live="polite">
        {chat.entries.map((entry) => (
          <Message key={entry.id} entry={entry} name={card.name} />
        ))}
        {chat.busy && (
          <article className="message message-char message-pending" data-testid="message-streaming">
            <div className="message-name">{card.name}</div>
            {chat.streaming ? (
              <div className="message-body" dangerouslySetInnerHTML={{ __html: streamingHtml }} />
            ) : (
              <div className="message-wait">{t("chatReplying", { name: card.name })}</div>
            )}
          </article>
        )}
        <div ref={endRef} />
      </div>

      {chat.failover && (
        <p className="chat-notice" role="status">
          {t(chat.failover.retried ? "chatFailoverRetried" : "chatFailover", {
            from: chat.failover.from,
            to: chat.failover.to,
          })}
        </p>
      )}
      {chat.error && (
        <p className="chat-error" role="alert">
          {chat.error}
        </p>
      )}

      <form
        className="composer"
        onSubmit={(event) => {
          event.preventDefault();
          void chat.send();
        }}
      >
        <textarea
          value={chat.input}
          placeholder={t("chatPlaceholder")}
          rows={2}
          onChange={(event) => chat.setInput(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
              event.preventDefault();
              void chat.send();
            }
          }}
        />
        {chat.busy ? (
          <button type="button" className="danger" onClick={chat.stop}>
            {t("chatStop")}
          </button>
        ) : (
          <button type="submit" className="primary" disabled={blocked || chat.input.trim() === ""}>
            {t("chatSend")}
          </button>
        )}
      </form>
    </section>
  );
}
