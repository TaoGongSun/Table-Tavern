// 卡片自帶介面的覆蓋層：多卡分頁、換幕與關閉鈕、那支沙盒 iframe、底部輸入列（含打字提示）。純 markup——
// 「要不要掛上去」的條件留在 App（只在遊玩畫面出現），殼內容與生成狀態各由自己的 controller 擁有。
import { type FormEvent, type ReactNode, useCallback, useEffect } from "react";
import { t } from "../../i18n";
import { IconClose, IconSceneAdvance, IconSend, IconStop } from "../../shared/ui/icons";
import { type CardChat } from "./card-chat-shim";
import { type CardMvu } from "./mvu/card-mvu-shim";
import { wouldOverflow, type SummaryBudget } from "../play/scene-budget";
import { ShellFrames } from "./shell-frames/ShellFrames";
import { slotWindow, type ShellRegistry } from "./shell-frames/shell-frames-state";

/** 底部輸入列：與聊天輸入框共用草稿與送出（App 包過桌面鎖的 send），停用條件由呼叫端照聊天輸入框算好 */
export interface OverlayComposer {
  input: string;
  onInputChange: (value: string) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  placeholder: string;
  /** 輸入框停用（與聊天輸入框同條件） */
  inputDisabled: boolean;
  /** 送出鈕停用（再加上換幕容量滿、去空白後為空） */
  sendDisabled: boolean;
  /** 生成中：送出鈕換成停止鈕 */
  canStop: boolean;
  onStop: () => void;
  /** 錯誤訊息（介面蓋住了聊天輸入框上方那一格，改在這裡顯示） */
  errorNote: ReactNode;
  /** 換幕容量滿的提示（與聊天輸入框上方同一句）；null＝不顯示 */
  notice: string | null;
}

/**
 * 照聊天輸入框（PlayView）組出底部輸入列：停用條件、placeholder、送出擋法一模一樣，草稿與送出用同一份。
 * send 是 App 包過桌面鎖的那支（打字送出路徑：失敗收回放回原文、換幕容量滿放回原文都在裡面）。
 */
export function buildOverlayComposer(input: {
  locked: boolean;
  speaker: string;
  castEmpty: boolean;
  /** chat.busy || 桌面操作中 */
  busy: boolean;
  sceneSummary: SummaryBudget | null | undefined;
  targetName: string;
  draft: string;
  setDraft: (value: string) => void;
  send: (event: FormEvent<HTMLFormElement>) => Promise<void>;
  canStop: boolean;
  onStop: () => void;
  errorNote: ReactNode;
}): OverlayComposer {
  const { locked, speaker, castEmpty, busy, draft } = input;
  const inputDisabled = locked || (!speaker && castEmpty) || busy;
  const fullForSend = !locked && wouldOverflow(input.sceneSummary, draft);
  const fullForActions = !locked && wouldOverflow(input.sceneSummary, "");
  return {
    input: draft,
    onInputChange: input.setDraft,
    onSubmit: (event) => {
      if (locked || fullForSend) {
        event.preventDefault();
        return;
      }
      void input.send(event);
    },
    placeholder: speaker
      ? t("composerPlaceholder", { name: input.targetName })
      : castEmpty
        ? t("composerNoCharacter")
        : t("composerNoTarget"),
    inputDisabled,
    sendDisabled: inputDisabled || fullForSend || draft.trim() === "",
    canStop: input.canStop,
    onStop: input.onStop,
    errorNote: input.errorNote,
    notice: fullForActions || fullForSend ? t("sceneCapacityFull") : null,
  };
}

interface CardInterfaceOverlayProps {
  /** 正在生成的那位要顯示的名字；null＝沒人在打字，輸入列上方的打字提示不出現 */
  generatingName: string | null;
  /** 介面殼的 HTML；null＝還沒備好，不掛 iframe */
  shellDoc: string | null;
  /** 殼指紋：每份殼一支新 iframe（雙緩衝翻面）；也是讀訊息推送的 token */
  shellKey: string;
  /** 顯示對象（桌、桌世代、殼所屬的卡）：換了就硬切、不雙緩衝 */
  shellIdentity: string;
  /** 兩支 iframe 的現況，宿主訊息監聽依它收卡片的行動與設定 */
  shellRegistry: ShellRegistry;
  /** 本場讀訊息快照；變動時與 iframe load 時推給沙盒 */
  chat: CardChat | null;
  /** MVU 變數快照；與讀訊息快照同一則推送 */
  mvu?: CardMvu | null;
  /** 換幕：與標題列那顆同一個動作與守門；介面整面蓋住標題列，玩家不必先關介面 */
  onAdvanceScene: () => void;
  /** 與標題列換幕鈕同一條件（生成中、沒有紀錄、桌面鎖住） */
  advanceDisabled: boolean;
  onClose: () => void;
  /** 分頁鈕：本場畫得出殼的卡（不到兩張就不顯示） */
  tabs?: { key: string; name: string }[];
  /** 目前顯示的是哪張卡 */
  activeCard?: string | null;
  onPickCard?: (key: string) => void;
  composer: OverlayComposer;
}

export function CardInterfaceOverlay({
  generatingName,
  shellDoc,
  shellKey,
  shellIdentity,
  shellRegistry,
  chat,
  mvu = null,
  onAdvanceScene,
  advanceDisabled,
  onClose,
  tabs = [],
  activeCard = null,
  onPickCard,
  composer,
}: CardInterfaceOverlayProps) {
  // 單向推送：只送到這份殼的那支 iframe（opaque origin 只能用 "*"），帶 token 讓舊殼與別桌的殼不收
  const push = useCallback(() => {
    if (chat === null) return;
    const target = [shellRegistry.pending, shellRegistry.front].find((slot) => slot?.key === shellKey);
    slotWindow(target ?? null)?.postMessage({ source: "table-tavern-host", kind: "chat", token: shellKey, chat, mvu }, "*");
  }, [chat, mvu, shellKey, shellRegistry]);
  // 逐字稿變了就推（本樓沒變、不換殼的情形）；殼回報 ready 時再推一次，補文件建好到殼初始化之間漏掉的更新
  useEffect(() => {
    push();
  }, [push]);
  const onReady = useCallback(
    (key: string) => {
      if (key === shellKey) push();
    },
    [push, shellKey],
  );

  return (
    <div className="card-interface-overlay">
      <div className="card-interface-toolbar">
        {tabs.length >= 2 && (
          <div className="btn-seg card-interface-tabs" role="group">
            {tabs.map((tab) => {
              const name = tab.name || t("cardInterfaceWorldTab");
              return (
                <button
                  key={tab.key}
                  type="button"
                  className={`btn btn-shrink card-interface-tab${tab.key === activeCard ? " is-active" : ""}`}
                  aria-pressed={tab.key === activeCard}
                  title={name}
                  onClick={() => onPickCard?.(tab.key)}
                >
                  <span className="btn-label">{name}</span>
                </button>
              );
            })}
          </div>
        )}
        <button
          type="button"
          className="btn btn-shrink"
          title={t("sceneAdvanceHint")}
          disabled={advanceDisabled}
          onClick={() => onAdvanceScene()}
        >
          <IconSceneAdvance />
          <span className="btn-label">{t("sceneAdvance")}</span>
        </button>
        <button
          type="button"
          className="btn btn-ghost btn-icon card-interface-close"
          aria-label={t("cardInterfaceClose")}
          title={t("cardInterfaceClose")}
          onClick={() => onClose()}
        >
          <IconClose />
        </button>
      </div>
      {/* 雙緩衝：新殼先在舊殼下面載，殼內墊片回報 ready 才翻面；首開與換桌／切卡直接繪 */}
      <ShellFrames
        doc={shellDoc}
        shellKey={shellKey}
        identity={shellIdentity}
        registry={shellRegistry}
        title={t("cardInterfaceOpen")}
        onReady={onReady}
      />
      {/* 底部輸入列：介面開著也能打自由行動（等同 ST 輸入框一直在），草稿與聊天輸入框同一份 */}
      <form className="card-interface-composer" onSubmit={composer.onSubmit}>
        {composer.notice && <p className="scene-length-hint scene-capacity-hint is-full">{composer.notice}</p>}
        {composer.errorNote && <div className="composer-error">{composer.errorNote}</div>}
        {/* 打字提示貼在輸入列正上方：介面看不到逐字串流，整段生成都顯示；放輸入列裡不蓋卡片內容 */}
        {generatingName !== null && (
          <div className="card-interface-status" role="status">
            <span className="card-interface-status-text">{t("typing", { name: generatingName })}</span>
            <span className="typing">
              <i />
              <i />
              <i />
            </span>
          </div>
        )}
        <div className="card-interface-composer-row">
          <input
            className="writebox"
            aria-label={t("composerAria")}
            value={composer.input}
            onChange={(event) => composer.onInputChange(event.currentTarget.value)}
            placeholder={composer.placeholder}
            disabled={composer.inputDisabled}
          />
          {composer.canStop ? (
            <button type="button" className="btn btn-primary" onClick={composer.onStop}>
              <IconStop />
              <span className="btn-label">{t("stopResponse")}</span>
            </button>
          ) : (
            <button type="submit" className="btn btn-primary" disabled={composer.sendDisabled}>
              <span className="btn-label">{t("send")}</span>
              <IconSend />
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
