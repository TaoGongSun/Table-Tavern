// 遊玩畫面：這一幕的訊息清單與底下的 composer。整支是受控元件——
// 逐字稿、生成狀態、輸入框與所有動作都由 chat controller 擁有，這裡只負責畫與回報。
import { FormEvent, ReactNode, useEffect, useLayoutEffect, useRef } from "react";
import { t } from "../i18n";
import { TranscriptEvent } from "../backend-contracts";
import { CharacterMeta } from "../card-model";
import { StoryText } from "./atoms";
import gmBook from "../assets/gm-book.png";

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
  requestReplyLabel: string;
  onUndoLast: () => void;
  onRequestReply: () => void;
  onGmNarrate: () => void;
  onGmAdvance: () => void;
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
  requestReplyLabel,
  onUndoLast,
  onRequestReply,
  onGmNarrate,
  onGmAdvance,
}: PlayViewProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLElement>(null);

  // 逐字稿整份換掉（切桌／換幕／分岔）或多一則：直接跳到底，不跑動畫。
  // 動畫在這裡會停在錯的位置——分岔是先掛載舊幕再換成新幕的紀錄，容器高度中途劇變，
  // smooth 捲到的是換掉前算出來的座標，玩家看到的是一片空白（scene-fork 實機驗收抓到）
  useLayoutEffect(() => {
    const list = listRef.current;
    if (list) list.scrollTop = list.scrollHeight;
  }, [events]);

  // 串流跟隨：這條高度是一個字一個字長的，用動畫才不會一跳一跳
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [generating, streamText]);

  // 換場提醒：粗估目前場景累計字元數，超過門檻就在送出鈕旁小字提醒（不擋操作）
  const sceneChars = events.reduce((sum, event) => sum + event.text.length, 0);
  const sceneTooLong = sceneChars > SCENE_LENGTH_HINT_CHARS;
  // 離開太久＋紀錄夠長才提醒換幕：兩者缺一，換幕都是白花一次摘要錢
  const showAwayHint = awayTooLong && sceneChars > SCENE_AWAY_HINT_MIN_CHARS;

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
                  <span className="pb-plate">{event.speaker_name}</span>
                </div>
                <StoryText text={event.text} />
              </div>
            );
          }
          return (
            <div key={index} className={`message message-${event.kind}`}>
              <StoryText text={event.text} />
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
        {canRestore && !busy && (
          <div className="undo-restore">
            <button type="button" onClick={() => onRestoreUndone()}>
              ↩ {t("undoRestore")}
            </button>
          </div>
        )}
        {/* 換幕的兩條補救路：只在這一幕還沒開始玩時出現，玩家一發言就自動收掉 */}
        {canUndoScene && (
          <div className="undo-restore">
            <button
              type="button"
              title={t("sceneSummaryRetryHint")}
              onClick={() => onRegenerateSummary()}
            >
              ↻ {t("sceneSummaryRetry")}
            </button>
            <button type="button" title={t("sceneRevertHint")} onClick={() => onRevertScene()}>
              ↩ {t("sceneRevert")}
            </button>
          </div>
        )}
        <div ref={bottomRef} />
      </section>

      {/* Composer 改整寬書寫面（ui-overhaul 拍板）：目標晶片只是把「點側欄選發言對象」既有狀態可見化 */}
      <form className="composer" onSubmit={onSubmit}>
        {speaker && (
          <div className="composer-opts">
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
              {targetName}
              <button
                type="button"
                className="opt-target-clear"
                aria-label={t("clearTarget")}
                title={t("clearTarget")}
                onClick={() => onClearTarget()}
              >
                ✕
              </button>
            </span>
          </div>
        )}
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
          disabled={(!speaker && castEmpty) || busy}
        />
        {/* 送出擺最左：它跟輸入框是同一件事，右邊那三顆是交給 AI 的動作
            （2026-07-28 使用者回報：送出在右下容易誤按成「請某某發言」） */}
        <div className="composer-send">
          <div className="composer-primary-action">
            <button type="submit" disabled={(!speaker && castEmpty) || busy}>
              {t("send")} ➤
            </button>
          </div>
          {/* 兩個換幕提醒只顯示一個：離開太久（快取已清）比紀錄長更急，優先出 */}
          {showAwayHint ? (
            <span className="scene-length-hint">{t("sceneAwayHint")}</span>
          ) : sceneTooLong ? (
            <span className="scene-length-hint">{t("sceneTooLongHint")}</span>
          ) : null}
          <div className="composer-ai-actions">
            <button
              className="undo-last"
              type="button"
              onClick={() => onUndoLast()}
              disabled={busy || events.length === 0}
              title={t("undoLastHint")}
            >
              ↩ {t("undoLast")}
            </button>
            <button
              className="request-reply"
              type="button"
              onClick={() => onRequestReply()}
              disabled={!speaker || busy}
              title={`${requestReplyLabel} — ${t("requestReplyHint")}`}
              aria-label={requestReplyLabel}
            >
              <span className="request-reply-label">{requestReplyLabel}</span>
            </button>
            <button type="button" onClick={onGmNarrate} disabled={busy} title={t("gmNarrateHint")}>
              {t("gmNarrate")}
            </button>
            <button
              type="button"
              onClick={onGmAdvance}
              disabled={busy || castEmpty}
              title={t("gmAdvanceHint")}
            >
              {t("gmAdvance")}
            </button>
          </div>
        </div>
      </form>
    </>
  );
}
