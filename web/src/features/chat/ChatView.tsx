import { useEffect, useMemo, useRef, useState } from "react";
import { renderHostMarkdown } from "../../shared/ui/host-markdown";
import { t } from "../../i18n";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { quotaBlocksSending } from "../funnel/quota";
import type { ChatEntry } from "./chat-turn";
import { displayText } from "./st-text";
import { useChat, type ChatController, type GameSetup } from "./useChat";

// 模型輸出與卡片文字一律走桌面版的安全渲染（DOMPurify），不直接塞 HTML（計畫 2.4）
function Message({ entry, html, name }: { entry: ChatEntry; html: string; name: string }) {
  return (
    <>
      {entry.role === "char" && <div className="message-name">{name}</div>}
      <div className="message-body" dangerouslySetInnerHTML={{ __html: html }} />
      {entry.interrupted && <div className="message-flag">{t("chatInterrupted")}</div>}
    </>
  );
}

/** 最後一則的動作列與就地編輯（按鈕在上、編輯框在下）。 */
function LastMessageTools({ chat, entry }: { chat: ChatController; entry: ChatEntry }) {
  const [draft, setDraft] = useState<string | null>(null);
  if (draft !== null) {
    return (
      <div className="message-edit">
        <div className="message-actions">
          <button
            type="button"
            className="primary"
            disabled={draft.trim() === ""}
            onClick={() => {
              chat.editLast(draft);
              setDraft(null);
            }}
          >
            {t("chatEditSave")}
          </button>
          <button type="button" className="ghost" onClick={() => setDraft(null)}>
            {t("chatEditCancel")}
          </button>
        </div>
        <textarea value={draft} rows={Math.min(12, draft.split("\n").length + 1)} onChange={(event) => setDraft(event.target.value)} />
      </div>
    );
  }
  return (
    <div className="message-actions" data-testid="last-actions">
      {chat.canRegenerate && (
        <button type="button" className="ghost" disabled={chat.busy} onClick={() => void chat.regenerate()}>
          {t("chatRegenerate")}
        </button>
      )}
      <button type="button" className="ghost" disabled={chat.busy} onClick={() => setDraft(entry.text)}>
        {t("chatEdit")}
      </button>
      <button type="button" className="ghost" disabled={chat.busy} onClick={chat.deleteLast}>
        {t("chatDelete")}
      </button>
    </div>
  );
}

export function ChatView({ game, session, onBack }: { game: GameSetup; session: OpenRouterSession; onBack: () => void }) {
  const chat = useChat(game, session);
  const endRef = useRef<HTMLDivElement>(null);
  const blocked = quotaBlocksSending(session.quota);
  const name = game.card.text.name;
  const streamingHtml = useMemo(() => renderHostMarkdown(chat.streaming), [chat.streaming]);
  // 顯示用 regex（markdownOnly）帶深度，每次逐字稿變動重算；存檔原文不變
  const rendered = useMemo(
    () => chat.entries.map((_, index) => renderHostMarkdown(displayText(chat.setup, chat.entries, index))),
    [chat.entries, chat.setup],
  );
  const lastIndex = chat.entries.length - 1;

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [chat.entries, chat.streaming]);

  return (
    <section className="chat">
      <div className="chat-bar">
        <button type="button" className="ghost" onClick={onBack} disabled={chat.busy}>
          ← {t("chatBack")}
        </button>
        <h2>{name}</h2>
        <span className="chat-note">{t("memoryOnlyNote")}</span>
      </div>

      <div className="chat-log" aria-live="polite">
        {chat.entries.map((entry, index) => (
          <article key={entry.id} className={`message message-${entry.role}`} data-testid={`message-${entry.role}`}>
            <Message entry={entry} html={rendered[index]} name={name} />
            {index === lastIndex && !chat.busy && <LastMessageTools key={entry.id + entry.text} chat={chat} entry={entry} />}
          </article>
        ))}
        {chat.busy && (
          <article className="message message-char message-pending" data-testid="message-streaming">
            <div className="message-name">{name}</div>
            {chat.streaming ? (
              <div className="message-body" dangerouslySetInnerHTML={{ __html: streamingHtml }} />
            ) : (
              <div className="message-wait">{t("chatReplying", { name })}</div>
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
