// 遊玩畫面：這一幕的訊息清單與底下的 composer。整支是受控元件——
// 逐字稿、生成狀態、輸入框與所有動作都由 chat controller 擁有，這裡只負責畫與回報。
import { FormEvent, ReactNode, useEffect, useLayoutEffect, useRef } from "react";
import { t } from "../../i18n";
import { TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { CharacterMeta } from "../characters/card-model";
import { StoryText } from "../../shared/ui/atoms";
import { eventDisplayText, speakerDisplayName } from "../../shared/ui/event-text";
import gmBook from "../../assets/gm-book.png";
import { IconSend, IconStop } from "../../shared/ui/icons";
import { useStickToBottom } from "../story-scroll/useStickToBottom";

// 換場提醒門檻：粗略以字元數估算紀錄長度，不精算 token。
// 快取上線後換幕不再省額度（摘要與換幕後首輪都全額計價，約等於連跑四輪），
// 提醒的理由改成「紀錄長到模型顧不上前面」，門檻從 8000 提到 30000（2026-08-04 實測拍板）。
const SCENE_LENGTH_HINT_CHARS = 30000;

// 離開太久的換幕提醒還要紀錄夠長才有意義：短紀錄重建本來就便宜，換幕反而多花一次摘要錢。
// 保溫仍照樣停在三次（那是省錢邏輯），這個門檻只決定要不要出聲提醒。
const SCENE_AWAY_HINT_MIN_CHARS = 8000;

// 串流中的旁白尾端會冒出狀態區塊，整則寫完才由後端剝乾淨；
// 這裡先切掉，免得玩家每回合都看到一段圍欄或標籤閃過去
function narrationStreamText(text: string) {
  const marker = text.search(/```|<details|<status|<UpdateVariable/i);
  return marker === -1 ? text : text.slice(0, marker);
}

interface PlayViewProps {
  /** 還沒填 API key 時的引導面板；元件本身留在 App */
  onboarding: ReactNode;
  /** 幕書籤上的文字（第 n 幕：幕名） */
  sceneLabel: string;
  events: TranscriptEvent[];
  /** 訊息作者的陣營色從這裡查 */
  metaOf: (id: string) => CharacterMeta | undefined;
  generating: { id: string; kind: "dialogue" | "narration" } | null;
  /** 生成中那位的名字與顏色（App 也拿去畫卡片介面的狀態條） */
  generatingMeta: CharacterMeta | undefined;
  streamText: string;
  busy: boolean;
  /** 唯讀：輸入與補救路全關，紀錄仍顯示 */
  locked?: boolean;
  canRestore: boolean;
  onRestoreUndone: () => void;
  /** 剛換完幕、還沒開始玩：重寫摘要與退回上一幕兩條補救路才出現 */
  canUndoScene: boolean;
  onRegenerateSummary: () => void;
  onRevertScene: () => void;
  /** 保溫連發到上限還沒等到玩家推進 */
  awayTooLong: boolean;
  /** 發言對象；空字串＝還沒選 */
  speaker: string;
  /** 對象是 GM：晶片換書皮、沒有角色卡可查 */
  gmTargeted: boolean;
  targetName: string;
  targetColor: string;
  /** 對象的頭像圖；沒有就退到 GM 書皮或角色 emoji */
  targetImage: string | null;
  targetEmoji: string;
  onClearTarget: () => void;
  input: string;
  onInputChange: (value: string) => void;
  /** 這桌一個在場角色都沒有：輸入框與 GM 推進都停用 */
  castEmpty: boolean;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  /** 對話或旁白生成中：原位的送出鍵改成停止 */
  canStop: boolean;
  onStop: () => void;
  /** 「請某某發言」帶名字的完整說法：按鈕只顯示固定短標，這句給 aria-label 與 title */
  requestReplyLabel: string;
  onUndoLast: () => void;
  onRequestReply: () => void;
  onGmNarrate: () => void;
  onGmAdvance: () => void;
  /** AI 錯誤訊息：跟換幕提醒同一處，限高內捲 */
  errorNote?: ReactNode;
}

export function PlayView({
  onboarding,
  sceneLabel,
  events,
  metaOf,
  generating,
  generatingMeta,
  streamText,
  busy,
  locked = false,
  canRestore,
  onRestoreUndone,
  canUndoScene,
  onRegenerateSummary,
  onRevertScene,
  awayTooLong,
  speaker,
  gmTargeted,
  targetName,
  targetColor,
  targetImage,
  targetEmoji,
  onClearTarget,
  input,
  onInputChange,
  castEmpty,
  onSubmit,
  canStop,
  onStop,
  requestReplyLabel,
  onUndoLast,
  onRequestReply,
  onGmNarrate,
  onGmAdvance,
  errorNote,
}: PlayViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLElement>(null);
  const markStuck = useStickToBottom(listRef);

  // 逐字稿整份換掉（切桌／換幕／分岔）或多一則：直接跳到底，不跑動畫。
  // 動畫在這裡會停在錯的位置——分岔是先掛載舊幕再換成新幕的紀錄，容器高度中途劇變，
  // smooth 捲到的是換掉前算出來的座標，玩家看到的是一片空白（scene-fork 實機驗收抓到）
  useLayoutEffect(() => {
    const list = listRef.current;
    if (list) list.scrollTop = list.scrollHeight;
    markStuck();
  }, [events, markStuck]);

  // 串流跟隨：這條高度是一個字一個字長的，用動畫才不會一跳一跳
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [generating, streamText]);

  // 換場提醒：粗估目前場景累計字元數，超過門檻就在輸入框上方小字提醒（不擋操作）
  const sceneChars = events.reduce((sum, event) => sum + event.text.length, 0);
  const sceneTooLong = !locked && sceneChars > SCENE_LENGTH_HINT_CHARS;
  // 離開太久＋紀錄夠長才提醒換幕：兩者缺一，換幕都是白花一次摘要錢
  const showAwayHint = !locked && awayTooLong && sceneChars > SCENE_AWAY_HINT_MIN_CHARS;
  const frozen = busy || locked;

  return (
    <>
      {onboarding}

      <section className="messages" aria-label={t("messagesAria")} ref={listRef}>
        {/* 幕書籤：目前這一幕的既有系統標籤（換幕／前幕／單幕匯出同一套資料） */}
        <div className="act-divider">
          <span className="act-tag">{sceneLabel}</span>
        </div>
        {events.map((event, index) => {
          if (event.kind === "dialogue" || event.kind === "player") {
            const meta = metaOf(event.speaker_id);
            const isPlayer = event.kind === "player";
            return (
              <div
                key={index}
                className={`message message-${event.kind}`}
                style={isPlayer ? undefined : { ["--fac" as string]: meta?.color ?? "#888888" }}
              >
                <div className="pb-name">
                  <span className="pb-plate">{speakerDisplayName(event)}</span>
                </div>
                <StoryText text={eventDisplayText(event)} />
                {event.truncated && <span className="response-truncated">{t("responseTruncated")}</span>}
              </div>
            );
          }
          return (
            <div key={index} className={`message message-${event.kind}`}>
              <StoryText text={eventDisplayText(event)} />
              {event.truncated && <span className="response-truncated">{t("responseTruncated")}</span>}
            </div>
          );
        })}
        {generating !== null && generating.kind === "dialogue" && (
          <div
            className="message message-dialogue"
            style={{ ["--fac" as string]: generatingMeta?.color ?? "#888888" }}
          >
            <div className="pb-name">
              <span className="pb-plate">{generatingMeta?.name ?? ""}</span>
            </div>
            {streamText ? (
              <span className="text">{streamText}</span>
            ) : (
              <span className="typing" aria-label={t("typing", { name: generatingMeta?.name ?? "" })}>
                <i />
                <i />
                <i />
              </span>
            )}
          </div>
        )}
        {generating !== null && generating.kind === "narration" && (
          <div className="message message-narration">
            {narrationStreamText(streamText) ? (
              <span className="text">{narrationStreamText(streamText)}</span>
            ) : (
              <span className="typing" aria-label={t("typing", { name: "GM" })}>
                <i />
                <i />
                <i />
              </span>
            )}
          </div>
        )}
        {/* 故事的補救動作集中在清單底部：收回、復原、換幕兩條補救路各依既有條件出現。
            容器本身清單空時也留著，補救鈕一出現不會把版面頂動 */}
        <div className="story-actions">
          {/* 收回上一句：清單空就沒東西可收；生成中或唯讀時停用但留位，不跳版 */}
          {events.length > 0 && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => onUndoLast()}
              disabled={frozen}
              title={t("undoLastHint")}
            >
              {t("undoLast")}
            </button>
          )}
          {canRestore && !frozen && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => onRestoreUndone()}
            >
              {t("undoRestore")}
            </button>
          )}
          {/* 換幕的兩條補救路：只在這一幕還沒開始玩時出現，玩家一發言就自動收掉 */}
          {!locked && canUndoScene && (
            <>
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                title={t("sceneSummaryRetryHint")}
                disabled={busy}
                onClick={() => onRegenerateSummary()}
              >
                {t("sceneSummaryRetry")}
              </button>
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                title={t("sceneRevertHint")}
                disabled={busy}
                onClick={() => onRevertScene()}
              >
                {t("sceneRevert")}
              </button>
            </>
          )}
        </div>
        <div ref={bottomRef} />
      </section>

      {/* 輸入區：發言對象晶片（有對象才有這排）→ 換幕提醒 → 整寬書寫面 → 動作列。
          目標晶片只是把「點側欄選發言對象」既有狀態可見化，角色名只出現在這裡 */}
      <form
        className="composer"
        onSubmit={(event) => {
          if (locked) {
            event.preventDefault();
            return;
          }
          onSubmit(event);
        }}
      >
        {speaker && (
          <div className="composer-target">
            <span
              className="opt-target"
              title={gmTargeted ? t("gmTargetHint") : t("castHint", { name: targetName })}
              style={{ ["--fac" as string]: targetColor }}
            >
              {gmTargeted ? (
                targetImage ? (
                  <img className="avatar-round opt-avatar gm-opt-avatar" src={targetImage} alt="" />
                ) : (
                  <img className="opt-avatar" src={gmBook} alt="" />
                )
              ) : targetImage ? (
                <img className="avatar-round opt-avatar" src={targetImage} alt="" />
              ) : (
                <span aria-hidden="true">{targetEmoji}</span>
              )}
              <span className="btn-label">{targetName}</span>
            </span>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              disabled={locked}
              onClick={() => onClearTarget()}
            >
              {t("clearTarget")}
            </button>
          </div>
        )}
        {/* 兩個換幕提醒只顯示一個：離開太久（快取已清）比紀錄長更急，優先出 */}
        {showAwayHint ? (
          <p className="scene-length-hint">{t("sceneAwayHint")}</p>
        ) : sceneTooLong ? (
          <p className="scene-length-hint">{t("sceneTooLongHint")}</p>
        ) : null}
        {errorNote && <div className="composer-error">{errorNote}</div>}
        <input
          className="writebox"
          aria-label={t("composerAria")}
          value={input}
          onChange={(e) => onInputChange(e.currentTarget.value)}
          placeholder={
            speaker
              ? t("composerPlaceholder", { name: targetName })
              : castEmpty
                ? t("composerNoCharacter")
                : t("composerNoTarget")
          }
          disabled={locked || (!speaker && castEmpty) || busy}
        />
        {/* 左邊相連的三顆是「交給 AI 的動作」，各自直接執行、不是選模式；右下是唯一的主按鈕。
            主次分級＋分處兩端，送出不會跟「請某某發言」相鄰誤按（2026-07-28 使用者回報的問題） */}
        <div className="composer-actions">
          <div className="btn-seg" role="group">
            <button
              type="button"
              className="btn"
              onClick={() => onRequestReply()}
              disabled={locked || !speaker || busy}
              title={`${requestReplyLabel} — ${t("requestReplyHint")}`}
              aria-label={`${t("requestReplyShort")} — ${requestReplyLabel}`}
            >
              <span className="btn-label">{t("requestReplyShort")}</span>
            </button>
            <button
              type="button"
              className="btn"
              onClick={onGmNarrate}
              disabled={frozen}
              title={t("gmNarrateHint")}
            >
              <span className="btn-label">{t("gmNarrate")}</span>
            </button>
            <button
              type="button"
              className="btn"
              onClick={onGmAdvance}
              disabled={frozen || castEmpty}
              title={t("gmAdvanceHint")}
            >
              <span className="btn-label">{t("gmAdvance")}</span>
            </button>
          </div>
          {/* 送出與停止同一個位置互換；兩種字都排進去、只顯示其一，按鈕寬度取兩者較大不跳動 */}
          {canStop ? (
            <button type="button" className="btn btn-primary composer-primary" onClick={onStop}>
              <span className="swap-label">
                <span>
                  <IconStop />
                  {t("stopResponse")}
                </span>
                <span className="swap-ghost" aria-hidden="true">
                  {t("send")}
                  <IconSend />
                </span>
              </span>
            </button>
          ) : (
            <button
              type="submit"
              className="btn btn-primary composer-primary"
              disabled={locked || (!speaker && castEmpty) || busy}
            >
              <span className="swap-label">
                <span>
                  {t("send")}
                  <IconSend />
                </span>
                <span className="swap-ghost" aria-hidden="true">
                  <IconStop />
                  {t("stopResponse")}
                </span>
              </span>
            </button>
          )}
        </div>
      </form>
    </>
  );
}
