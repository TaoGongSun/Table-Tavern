import { useEffect, useMemo, useRef, useState } from "react";
import { renderHostMarkdown } from "../../shared/ui/host-markdown";
import { t } from "../../i18n";
import type { OpenRouterSession } from "../openrouter/useOpenRouterSession";
import { quotaBlocksSending } from "../funnel/quota";
import type { ReleaseInfo } from "../funnel/releases";
import { downloadCard, downloadStChat, downloadWebSave } from "../saves/download";
import { ExportFunnel } from "../saves/ExportFunnel";
import type { SaveStore } from "../saves/save-store";
import { FrontendNotice } from "../card-interface/FrontendNotice";
import { splitFrontends } from "../card-interface/frontend-blocks";
import { MessageBody, type FrontendContext } from "../card-interface/MessageBody";
import { chatFloors, useFrontendHost } from "../card-interface/useFrontendHost";
import type { ChatEntry } from "./chat-turn";
import { displayText } from "./st-text";
import { useChat, type ChatController, type GameSetup } from "./useChat";

// 模型輸出與卡片文字一律走桌面版的安全渲染（DOMPurify），不直接塞 HTML（計畫 2.4）；前端介面另畫成沙盒 iframe
function Message({ entry, text, floor, name, frontends }: { entry: ChatEntry; text: string; floor: number; name: string; frontends: FrontendContext }) {
  return (
    <>
      {entry.role === "char" && <div className="message-name">{name}</div>}
      <MessageBody text={text} floor={floor} frontends={frontends} />
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

/** 匯出：網頁存檔（桌面版接著玩，匯完導流）與 SillyTavern 聊天檔（有損，附說明）。 */
export function ExportBar({ chat, name, release }: { chat: ChatController; name: string; release: ReleaseInfo }) {
  const [funnel, setFunnel] = useState(false);
  const [stHint, setStHint] = useState(false);
  // 錯誤存成「怎麼說」，繪製時才翻：換語系跟著換
  const [error, setError] = useState<(() => string) | null>(null);
  return (
    <div className="export-bar">
      <div className="export-actions">
        <button
          type="button"
          disabled={chat.busy}
          onClick={() => {
            let failed: string | null;
            try {
              failed = downloadWebSave(chat.exportSave(), name);
            } catch (reason) {
              failed = String(reason);
            }
            setError(() => (failed ? () => t("exportFailed", { detail: failed }) : null));
            setFunnel(!failed);
            setStHint(false);
          }}
        >
          {t("exportSave")}
        </button>
        <button
          type="button"
          className="ghost"
          disabled={chat.busy}
          title={t("exportStHint")}
          onClick={() => {
            downloadStChat(chat.exportStChat(), name);
            setStHint(true);
            setFunnel(false);
          }}
        >
          {t("exportStChat")}
        </button>
      </div>
      {stHint && (
        <div className="chat-notice" role="status" data-testid="export-st-hint">
          <p>{t("exportStHint")}</p>
          <button type="button" onClick={() => downloadCard(chat.setup.card)}>
            {t("exportCard")}
          </button>
        </div>
      )}
      {error && (
        <p className="chat-error" role="alert" data-testid="export-error">
          {error()}
        </p>
      )}
      {funnel && <ExportFunnel release={release} onClose={() => setFunnel(false)} />}
    </div>
  );
}

export function ChatView({
  game,
  session,
  saves = null,
  release,
  onBack,
}: {
  game: GameSetup;
  session: OpenRouterSession;
  saves?: SaveStore | null;
  release: ReleaseInfo;
  onBack: () => void;
}) {
  const chat = useChat(game, session, saves);
  const endRef = useRef<HTMLDivElement>(null);
  const blocked = quotaBlocksSending(session.quota);
  const name = game.card.text.name;
  const streamingHtml = useMemo(() => renderHostMarkdown(chat.streaming), [chat.streaming]);
  // 顯示用 regex（markdownOnly）帶深度，每次逐字稿變動重算；存檔原文不變
  // 酒館助手類巨集也在顯示時代換（讀當下的變數）
  const display = chat.mvu.display;
  const shown = useMemo(
    () => chat.entries.map((_, index) => display(displayText(chat.setup, chat.entries, index), chat.entries)),
    [chat.entries, chat.setup, display],
  );
  // 卡片介面：按鈕送出的句子直接送（回合中不送）、存的設定跟著存檔
  const host = useFrontendHost({
    onInput: (text) => void chat.sendText(text),
    onStorage: chat.setCardStorage,
    mvu: { write: (_frame, data, reply) => chat.mvu.write(data, reply), evaluate: (_frame, data, reply) => chat.mvu.evaluate(data, reply) },
  });
  const userName = chat.setup.userName;
  const floors = useMemo(() => chatFloors(chat.entries, userName, name), [chat.entries, userName, name]);
  const unsupported = game.card.view.interface.unsupported !== null;
  const mvuBase = chat.mvu.frontend;
  const frontends = useMemo<FrontendContext>(
    () => ({
      host,
      floors,
      mvuFor: (floor) => (mvuBase === null ? null : { ...mvuBase, currentId: floor }),
      storage: chat.cardStorage,
      unsupported,
    }),
    [host, floors, mvuBase, chat.cardStorage, unsupported],
  );
  const hasFrontend = useMemo(() => !unsupported && shown.some((text) => splitFrontends(text).some((segment) => segment.kind === "frontend")), [shown, unsupported]);
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
        <span className="chat-note" data-testid="autosave-note">
          {saves ? t("autosaveNote") : t("savesErr_storage")}
        </span>
      </div>
      <ExportBar chat={chat} name={name} release={release} />
      {chat.mvu.initError !== null && (
        <p className="chat-error" role="alert" data-testid="mvu-init-failed">
          {t("mvuInitFailed", { comment: chat.mvu.initError })}
        </p>
      )}
      {chat.saveFailed && (
        <p className="chat-error" role="alert">
          {t("autosaveFailed")}
        </p>
      )}

      {hasFrontend && <FrontendNotice />}
      <div className="chat-log" aria-live="polite">
        {chat.entries.map((entry, index) => (
          <article key={entry.id} className={`message message-${entry.role}`} data-testid={`message-${entry.role}`}>
            <Message entry={entry} text={shown[index]} floor={index} name={name} frontends={frontends} />
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
