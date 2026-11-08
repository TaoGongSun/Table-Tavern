// 行動格（協定 §3.3）：草稿節流上傳（不公開）、送出即公開、收回成草稿、交給 GM（協定的 skip）。GM 生成中可打字不可送；
// 被拒（回合已往下走、GM 還在說）時草稿原樣保留。交給 AI 代管中照樣能打字存草稿，只是送不出（不在名單上）。
// 房主當場開的倒數在輸入框上方出一條橫條（房間設定一開始就有的倒數不出）；送出、交給 GM 或停下就收。
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { flushSync } from "react-dom";
import { DicePicker } from "@desktop/features/dice/DicePicker";
import diceCopy from "@desktop/features/dice/dice-copy.json";
import { rankSuggestions } from "@desktop/features/dice/dice-suggestions";
import { currentLang, t, type MsgKey } from "../../i18n";
import { errorText } from "./error-text";
import type { RoomView } from "./room-state";
import { useRun, type Act } from "./useRoom";

const ROLLED_FORMULA = / (\d+d\d+(?:[+-]\d+)?): \[[^\]]*\](?: [+-]\d+)? = -?\d+$/;

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
  below,
}: {
  view: RoomView;
  act: Act;
  sendDraft: (text: string) => void;
  cancelDraft: () => void;
  leading?: ReactNode;
  compact?: boolean;
  urgent?: boolean;
  onTyping?: (typing: boolean) => void;
  /** 動作列正下方（房主：已送出的行動會保留那行小字） */
  below?: ReactNode;
}) {
  const area = useRef<HTMLTextAreaElement>(null);
  // 每個觸發每回合最多一次；同一回合又手動收起就留著
  const fired = useRef({
    round: -1,
    urgent: false,
    returned: false,
    returnText: null as string | null,
    nudge: 0, // 已處理過的催促次數（view.nudged.n）
    timer: -1, // 已震過的回合（房主當場開倒數，每回合一次）
  });
  const [text, setText] = useState(view.me.draft ?? "");
  const [tucked, setTucked] = useState(false); // 只在記憶體：進房一律展開
  const [taken, setTaken] = useState(false); // 房主收回、原文已直接放回輸入框：送出前提示一行
  const [tooLong, setTooLong] = useState(false);
  const [stuck, setStuck] = useState(false); // 自動放回沒存成：原文照留在下面那格
  const { run: send, error } = useRun(act);
  const [rollError, setRollError] = useState<string | null>(null);
  const box = useRef<HTMLDivElement>(null);
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
  // 房主 GM 回合期間：之前送出的行動留著，GM 說完放回名單
  const carried =
    r?.phase !== "acting" ? r?.carry?.find((e) => e.memberId === view.me.memberId) : undefined;
  const shown = acting ? entry : carried;
  // 交給 GM 期間：輸入框唯讀（草稿留在框裡），收回後恢復可編輯
  const handed = shown?.status === "skipped";
  const nudged = openTurn && view.nudged?.round === r?.round;
  // 房主自己開的倒數不必通知自己（回合列已有「停止倒數」與剩餘時間）
  const timerOn =
    openTurn && view.timerStarted === r?.round && view.room.hostId !== view.me.memberId;
  const timerMinutes = (() => {
    if (!timerOn || !r) return 0;
    const ms =
      r.deadline !== null ? Math.max(0, r.deadline - Date.now()) : (r.pausedRemainingMs ?? 0);
    return Math.round(ms / 60_000);
  })();
  const live = view.room.status === "playing" || view.room.status === "transition";
  const dice = diceCopy[currentLang()];
  const myName = view.cards.find((c) => c.cardId === view.me.cardId)?.name;
  const gmText = [...view.story].reverse().find((p) => p.speaker.kind === "gm")?.text ?? "";
  const castNames = view.cards.map((c) => c.name).join("\n");
  const ranked = useMemo(
    () => rankSuggestions(gmText, myName, castNames.split("\n")),
    [gmText, myName, castNames],
  );
  const myRolls = view.story.filter((p) => p.rollBy === view.me.memberId && p.round === r?.round);
  const rolled = myRolls.flatMap((p) => ROLLED_FORMULA.exec(p.text)?.[1] ?? []);
  // 擲過的換下一個；自己的擲完就退回通用「擲骰」，不落到別人的算式
  const primary = ranked.find((x) => !x.other && !rolled.includes(x.value))?.value ?? null;
  const rollReason = !view.executor.diceEnabled
    ? t(view.room.hostId === view.me.memberId ? "rollOffHost" : "rollOff")
    : r?.phase !== "acting"
      ? t("rollWaitGm")
      : !entry
        ? t("rollNotOnRoster")
        : done
          ? t("rollSubmitted")
          : myRolls.length >= 8
            ? t("rollLimit")
            : "";
  /** 輸入框空著就一起送出（比照單人：空框擲骰接著跑 GM）；有字不送。被拒一律講出來，不靜默 */
  const rollDice = async (formula: string) => {
    const submit = !text.trim();
    if (submit) cancelDraft();
    const reply = await act("action.roll", {
      round: r?.round ?? 0,
      formula,
      ...(submit ? { submit } : {}),
    });
    setRollError(
      reply.ok
        ? null
        : reply.code === "E_TEXT_INVALID"
          ? dice.badFormula
          : (errorText(reply.code, reply.retryAfterMs) ?? t("rollRejected")),
    );
  };
  // 展開區要看得到：手機鍵盤彈起時故事區變矮，把整格捲回視窗內
  const onRollExpand = useCallback(() => box.current?.scrollIntoView?.({ block: "end" }), []);
  const diceButton = (
    <span className="action-dice">
      <DicePicker
        copy={dice}
        options={ranked.map((x) => x.value)}
        primary={primary}
        rolled={rolled}
        reason={rollReason}
        quiet
        buttonClass={compact ? "ghost small" : "ghost"}
        onRoll={rollDice}
        onExpand={onRollExpand}
      />
    </span>
  );
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
  // 房主催促（每催一次一回）：還沒送出才理；瀏覽器支援才震（iOS Safari 不支援），收起就展開，輸入框取得焦點
  useEffect(() => {
    const n = view.nudged?.n ?? 0;
    if (n === fired.current.nudge) return;
    fired.current.nudge = n;
    if (!nudged) return;
    navigator.vibrate?.(200);
    setTucked(false);
    setTimeout(() => area.current?.focus());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view.nudged?.n]);
  // 房主當場開倒數：跟催促一樣，每回合只震一次、收起的輸入格展開一次（橫條在收起分支裡看不到）；瀏覽器不支援震動就只有橫條
  useEffect(() => {
    const roundNo = r?.round;
    if (!timerOn || roundNo == null || fired.current.timer === roundNo) return;
    fired.current.timer = roundNo;
    navigator.vibrate?.(200);
    setTucked(false);
  }, [timerOn, r?.round]);
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
      readOnly={handed}
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
  const errorLine = (error || rollError) && (
    <div className="field-error" data-testid="action-error">
      {error || rollError}
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
      title={t("actionSkipTitle")}
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
        : carried
          ? t("actionCarried")
          : r?.phase !== "acting"
            ? t("errRoundNotActing")
            : !entry
              ? t("actionNextRound")
              : entry?.status === "skipped"
                ? t("actionHanded")
                : done && !entry?.text
                  ? t("actionDone")
                  : null;
  // 房主催促：醒目橫條緊貼輸入框上方；送出、交給 GM、回合結束任一發生就消失
  const nudgeBar = nudged && (
    <div className="nudge-bar" role="status" data-testid="nudged-note">
      ⏰ {t("actionNudged")}
    </div>
  );
  const timerBar = timerOn && (
    <div className="nudge-bar" role="status" data-testid="host-timer-note">
      {t("actionHostTimer", { n: timerMinutes })}
    </div>
  );
  return (
    <div className="action-box" data-testid="action-box" ref={box}>
      {hint && <div className={acting && handed ? "action-status" : "action-hint"}>{hint}</div>}
      {/* 送出後輸入框清空：寫出送了什麼，免得以為沒送出又打一次（原文就在名單上，結算後消失） */}
      {shown?.status === "submitted" && shown.text && (
        <div className="action-status" data-testid="submitted-note">
          {t("actionSubmittedNote", {
            text:
              [...shown.text].length > 20
                ? `${[...shown.text].slice(0, 20).join("")}…`
                : shown.text,
          })}
        </div>
      )}
      {acting && done && view.kept === r?.round && (
        <div className="action-hint" data-testid="kept-note">
          {t("actionKeptNote")}
        </div>
      )}
      {acting && !done && myRolls.length > 0 && (
        <div className="action-hint" data-testid="roll-kept-note">
          {t("rollKeptNote")}
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
            <>
              {timerBar}
              {nudgeBar}
              <div className="action-row">
                {input}
                {retractable ? retract : submit}
              </div>
            </>
          )}
          {errorLine}
          {/* 收起時跳過跟著輸入列走；下面留 GM 與骰鈕，要跳過先點細列展開 */}
          <div className="action-buttons action-minor">
            {leading && <span className="action-leading">{leading}</span>}
            {diceButton}
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
          {below}
        </>
      ) : (
        <>
          {timerBar}
          {nudgeBar}
          {input}
          {errorLine}
          <div className="action-buttons">
            {leading && <span className="action-leading">{leading}</span>}
            {diceButton}
            {retractable ? (
              retract
            ) : (
              <>
                {skip}
                {submit}
              </>
            )}
          </div>
          {below}
        </>
      )}
    </div>
  );
}
