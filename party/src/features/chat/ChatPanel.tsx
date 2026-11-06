// 聊天頁：舊到新、自動捲到最新。對象預設大家；送過私訊後維持那個人，直到自己改。
// 一直掛著、切到別頁只用 hidden 藏起來：對象與沒送的字不因切頁消失。送出只清那一次送的內容，等回覆期間新打的字留著。
// 桌面嵌入 Enter 送出、Shift+Enter 換行；輸入法組字中的 Enter 不攔。手機 Enter 換行，只用送出鈕。
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { chatOption, chatWho } from "./chat-label";
import "./chat.css";

const LIMIT = 500;
const chars = (s: string) => [...s.replace(/\r\n/g, "\n").normalize("NFC")].length;

function clock(at: number) {
  const d = new Date(at);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}`;
}

const composing = (e: KeyboardEvent<HTMLTextAreaElement>) =>
  e.nativeEvent.isComposing || e.keyCode === 229;

export function ChatPanel({
  view,
  act,
  desktop,
  hidden,
}: {
  view: RoomView;
  act: Act;
  desktop: boolean;
  hidden: boolean;
}) {
  const list = useRef<HTMLDivElement>(null);
  const area = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState("");
  const [target, setTarget] = useState("");
  const [sending, setSending] = useState(false);
  const { run, error } = useRun(act);
  const others = view.members.filter((m) => m.memberId !== view.me.memberId);
  useEffect(() => {
    if (
      target &&
      !view.members.some((m) => m.memberId === target && m.memberId !== view.me.memberId)
    )
      setTarget("");
  }, [view.members, view.me.memberId, target]);
  const n = chars(text);
  const over = n > LIMIT;
  const canSend = n >= 1 && !over && !sending;
  const submit = async () => {
    if (!canSend) return;
    const sent = text;
    setSending(true);
    const ok = await run("chat.send", target ? { text: sent, to: target } : { text: sent });
    setSending(false);
    // 只清這次送出的內容：等回覆期間接著打的字留著
    if (ok) setText((cur) => (cur.startsWith(sent) ? cur.slice(sent.length) : cur));
  };
  useLayoutEffect(() => {
    const el = list.current;
    if (el && !hidden) el.scrollTop = el.scrollHeight;
  }, [view.chat, hidden]);
  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight + 2}px`;
  }, [text]);
  const targetName = target ? others.find((m) => m.memberId === target)?.nickname : null;
  return (
    <div className="chat-panel" hidden={hidden}>
      <div className="chat-list" data-testid="chat-list" ref={list}>
        {view.chat.length === 0 && <p className="muted chat-empty">{t("chatEmpty")}</p>}
        {view.chat.map((m) => (
          <div
            key={m.id}
            className={m.to === null ? "chat-msg" : "chat-msg private"}
            data-testid="chat-msg"
          >
            <div className="chat-meta">
              <span className="part-name">
                {t("chatDir", { from: chatWho(view, m.from), to: chatWho(view, m.to) })}
                {m.to !== null && t("chatPrivate")}
              </span>
              <time className="chat-time" dateTime={new Date(m.at).toISOString()}>
                {clock(m.at)}
              </time>
            </div>
            <div className="chat-text">{m.text}</div>
          </div>
        ))}
      </div>
      <form
        className="chat-compose"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="action-row">
          <select value={target} data-testid="chat-to" onChange={(e) => setTarget(e.target.value)}>
            <option value="">{t("chatToAll")}</option>
            {others.map((m) => (
              <option key={m.memberId} value={m.memberId}>
                {chatOption(view, m.memberId, m.nickname)}
              </option>
            ))}
          </select>
          <textarea
            ref={area}
            rows={1}
            value={text}
            data-testid="chat-text"
            placeholder={
              targetName ? t("chatPlaceholderTo", { name: targetName }) : t("chatPlaceholder")
            }
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (!desktop || e.key !== "Enter" || e.shiftKey || composing(e)) return;
              e.preventDefault();
              void submit();
            }}
          />
          <button type="submit" data-testid="chat-send" disabled={!canSend}>
            {t("chatSend")}
          </button>
        </div>
        {over && (
          <div className="field-error" data-testid="chat-over">
            {t("chatRemain", { n: LIMIT - n })}
          </div>
        )}
        {error && (
          <div className="field-error" data-testid="chat-error">
            {error}
          </div>
        )}
      </form>
    </div>
  );
}
