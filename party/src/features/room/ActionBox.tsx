// 行動格（協定 §3.3）：草稿節流上傳（不公開）、送出即公開、收回成草稿、跳過。GM 生成中可打字不可送；
// 被拒（回合已往下走、GM 還在說）時草稿原樣保留。交給 AI 代管中照樣能打字存草稿，只是送不出（不在名單上）。
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";
import { t, type MsgKey } from "../../i18n";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

const STATUS: Record<string, MsgKey> = {
  pending: "statusPending",
  drafting: "statusDrafting",
  submitted: "statusSubmitted",
  skipped: "statusSkipped",
};

/** `leading`：動作列左側（房主的 GM 鈕，照單人版跟送出同一列）。
 *  `compact`（手機）：聊天式——輸入框一行起跳、隨內容長高，主鈕（送出／收回修改）在輸入框右邊，跳過與 GM 鈕縮成下面一條小鈕列。
 *  可手動收成一條細列（不卸載，草稿照留）；`urgent` 為倒數最後 30 秒。`onTyping` 讓房間在打字時暫停往回捲收起。 */
export function ActionBox({
  view,
  act,
  sendDraft,
  cancelDraft,
  leading,
  compact = false,
  urgent = false,
  onTyping,
}: {
  view: RoomView;
  act: Act;
  sendDraft: (text: string) => void;
  cancelDraft: () => void;
  leading?: ReactNode;
  compact?: boolean;
  urgent?: boolean;
  onTyping?: (typing: boolean) => void;
}) {
  const area = useRef<HTMLTextAreaElement>(null);
  // 每個觸發每回合最多一次；同一回合又手動收起就留著
  const fired = useRef({
    round: -1,
    urgent: false,
    returned: false,
    returnText: null as string | null,
  });
  const [text, setText] = useState(view.me.draft ?? "");
  const [tucked, setTucked] = useState(false); // 只在記憶體：進房一律展開
  const [taken, setTaken] = useState(false); // 房主收回、原文已直接放回輸入框：送出前提示一行
  const [tooLong, setTooLong] = useState(false);
  const [stuck, setStuck] = useState(false); // 自動放回沒存成：原文照留在下面那格
  const { run: send, error } = useRun(act);
  useEffect(() => {
    if (view.me.draft !== null) setText(view.me.draft); // 收回、換幕退回的原文
  }, [view.me.draft]);
  const returned = view.me.returned;
  useEffect(() => {
    // 房主收回的原文：輸入框空著就直接放回去（重連、重開頁面同一條規則）；有草稿就留在下面那格讓他自己帶回或丟掉
    if (!returned || text.trim()) return;
    void bring(returned, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [returned]);
  /** 帶回：草稿跟清原文同一筆交易，伺服器存好才換輸入框；存不進去就原文照留在下面那格 */
  const bring = async (next: string, auto = false) => {
    cancelDraft();
    if (!(await send("action.returned_dismiss", { draft: next }))) {
      if (text && live && !done) sendDraft(text); // 上面取消了還沒上傳的草稿：沒放回就照原樣補送，免得丟掉最後打的字
      if (auto) setStuck(true);
      else setTooLong(true);
      return;
    }
    setText(next);
    setTooLong(false);
    setStuck(false);
    if (auto) setTaken(true);
  };
  const r = view.round;
  const entry = r?.roster.find((e) => e.memberId === view.me.memberId);
  const acting = r?.phase === "acting" && !!entry;
  const done = entry?.status === "submitted" || entry?.status === "skipped";
  const openTurn = acting && !done;
  const live = view.room.status === "playing" || view.room.status === "transition";
  // 生成中、不在名單上、已送出時不展開
  useEffect(() => {
    if (!compact) return;
    const f = fired.current;
    const roundNo = r?.round ?? 0;
    let open = false;
    if (f.round !== roundNo) {
      f.round = roundNo;
      f.urgent = false;
      f.returned = false;
      if (openTurn) open = true;
    }
    if (openTurn && urgent && !f.urgent) {
      f.urgent = true;
      open = true;
    }
    const token = returned ?? null;
    if (token !== f.returnText) {
      if (!token) f.returnText = null;
      else if (openTurn && !f.returned) {
        f.returnText = token;
        f.returned = true;
        open = true;
      }
    }
    if (open) setTucked(false);
  }, [compact, r?.round, openTurn, urgent, returned]);
  useEffect(() => () => onTyping?.(false), [onTyping]);
  const run = async (tt: string, p: Record<string, unknown> = {}) => {
    await send(tt, { round: r?.round ?? 0, ...p });
  };
  /** 送出／跳過：先取消待送草稿；送出成功清空輸入框（伺服器也清了草稿），否則把框裡的字重新存成草稿 */
  const commit = async (tt: string, p: Record<string, unknown> = {}) => {
    cancelDraft();
    const ok = await send(tt, { round: r?.round ?? 0, ...p });
    if (ok && tt === "action.submit") {
      setText("");
      setTaken(false);
    } else if (text) sendDraft(text);
  };
  // 手機輸入框隨內容長高（上限在 App.css 的 max-height，超過就框內捲）
  useLayoutEffect(() => {
    const el = area.current;
    if (!compact || !el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight + 2}px`;
  }, [compact, text, tucked]);
  const input = (
    <textarea
      ref={area}
      value={text}
      rows={compact ? 1 : 3}
      maxLength={2000}
      placeholder={t("actionPlaceholder")}
      data-testid="action-text"
      onFocus={() => onTyping?.(true)}
      onBlur={() => onTyping?.(false)}
      onChange={(e) => {
        setText(e.target.value);
        if (live && !done) sendDraft(e.target.value);
      }}
    />
  );
  const errorLine = error && (
    <div className="field-error" data-testid="action-error">
      {error}
    </div>
  );
  // 「收回修改」只在行動中：結算後（GM 生成中、卡住）收不回，照常給停用的跳過／送出，不留一顆一直灰著的收回
  const retractable = done && acting;
  const retract = (
    <button type="button" onClick={() => run("action.retract")}>
      {t("actionRetract")}
    </button>
  );
  const skip = (
    <button
      type="button"
      className={compact ? "ghost small action-skip" : "ghost"}
      disabled={!acting}
      onClick={() => commit("action.skip")}
    >
      {t("actionSkip")}
    </button>
  );
  const submit = (
    <button
      type="button"
      className="primary"
      disabled={!acting || !text.trim()}
      onClick={() => commit("action.submit", { text })}
    >
      {t("actionSubmit")}
    </button>
  );
  const hint = !live
    ? t("actionWaitStart")
    : view.me.delegated
      ? t("actionDelegated")
      : r?.phase === "held"
        ? t("actionHeld")
        : r?.phase !== "acting"
          ? t("errRoundNotActing")
          : !entry
            ? t("actionNextRound")
            : done && !(entry?.status === "submitted" && entry.text)
              ? t("actionDone")
              : null;
  return (
    <div className="action-box" data-testid="action-box">
      {hint && <div className="action-hint">{hint}</div>}
      {/* 送出後輸入框清空：寫出送了什麼，免得以為沒送出又打一次（原文就在名單上，結算後消失） */}
      {r?.phase === "acting" && entry?.status === "submitted" && entry.text && (
        <div className="action-hint" data-testid="submitted-note">
          {t("actionSubmittedNote", {
            text:
              [...entry.text].length > 20
                ? `${[...entry.text].slice(0, 20).join("")}…`
                : entry.text,
          })}
        </div>
      )}
      {taken && (
        <div className="action-hint" data-testid="returned-notice">
          {t("returnedNotice")}
        </div>
      )}
      {returned && (text.trim() || stuck) && (
        // 房主收回的行動原文：只給本人；已有草稿時不覆蓋，帶回接到草稿尾端
        <div className="returned" data-testid="returned">
          <div className="part-name">{t("returnedTitle")}</div>
          <div className="part-text">{returned}</div>
          <div className="action-buttons">
            <button
              type="button"
              onClick={() => void bring(text ? `${text}\n${returned}` : returned)}
            >
              {t("returnedBring")}
            </button>
            <button
              type="button"
              className="ghost"
              onClick={() => void send("action.returned_dismiss")}
            >
              {t("returnedDrop")}
            </button>
          </div>
          {tooLong && (
            <div className="field-error" data-testid="returned-too-long">
              {t("returnedTooLong")}
            </div>
          )}
        </div>
      )}
      {compact ? (
        <>
          {tucked ? (
            <button
              type="button"
              className="dock-strip action-untuck"
              data-testid="action-untuck"
              onClick={() => {
                // iOS 要在這次點擊裡 focus 才會跳出鍵盤
                flushSync(() => setTucked(false));
                area.current?.focus();
              }}
            >
              <span>{text.trim() ? t("actionDraft") : t("actionWrite")}</span>
              {(entry ? t(STATUS[entry.status]) : hint) && (
                <small>{entry ? t(STATUS[entry.status]) : hint}</small>
              )}
            </button>
          ) : (
            <div className="action-row">
              {input}
              {retractable ? retract : submit}
            </div>
          )}
          {errorLine}
          {/* 收起時跳過跟著輸入列走；下面只留 GM 列，要跳過先點細列展開 */}
          {(leading || !tucked) && (
            <div className="action-buttons action-minor">
              {leading && <span className="action-leading">{leading}</span>}
              {!tucked && (
                <button
                  type="button"
                  className="ghost small action-tuck"
                  data-testid="action-tuck"
                  onClick={() => setTucked(true)}
                >
                  {t("actionFold")}
                </button>
              )}
              {!tucked && !retractable && skip}
            </div>
          )}
        </>
      ) : (
        <>
          {input}
          {errorLine}
          <div className="action-buttons">
            {leading && <span className="action-leading">{leading}</span>}
            {retractable ? (
              retract
            ) : (
              <>
                {skip}
                {submit}
              </>
            )}
          </div>
        </>
      )}
    </div>
  );
}
