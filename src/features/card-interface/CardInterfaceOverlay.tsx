// 卡片自帶介面的覆蓋層：打字狀態列、換幕與關閉鈕、那支沙盒 iframe。純 markup——
// 「要不要掛上去」的條件留在 App（只在遊玩畫面出現），殼內容與生成狀態各由自己的 controller 擁有。
import { useCallback, useEffect, useRef } from "react";
import { t } from "../../i18n";
import { IconClose, IconSceneAdvance } from "../../shared/ui/icons";
import { type CardChat } from "./card-chat-shim";
import { type CardMvu } from "./mvu/card-mvu-shim";

interface CardInterfaceOverlayProps {
  /** 正在生成的那位要顯示的名字；null＝沒人在打字，狀態列不出現 */
  generatingName: string | null;
  /** 介面殼的 HTML；null＝還沒備好，不掛 iframe */
  shellDoc: string | null;
  /** 殼指紋：換一份殼就讓整支 iframe 重掛；也是讀訊息推送的 token */
  shellKey: string;
  /** 本場讀訊息快照；變動時與 iframe load 時推給沙盒 */
  chat: CardChat | null;
  /** MVU 變數快照；與讀訊息快照同一則推送 */
  mvu?: CardMvu | null;
  /** 換幕：與標題列那顆同一個動作與守門；介面整面蓋住標題列，玩家不必先關介面 */
  onAdvanceScene: () => void;
  /** 與標題列換幕鈕同一條件（生成中、沒有紀錄、桌面鎖住） */
  advanceDisabled: boolean;
  onClose: () => void;
}

export function CardInterfaceOverlay({
  generatingName,
  shellDoc,
  shellKey,
  chat,
  mvu = null,
  onAdvanceScene,
  advanceDisabled,
  onClose,
}: CardInterfaceOverlayProps) {
  const frameRef = useRef<HTMLIFrameElement | null>(null);
  // 單向推送：只送到目前這支 iframe（opaque origin 只能用 "*"），帶 token 讓舊殼與別桌的殼不收
  const push = useCallback(() => {
    if (chat === null) return;
    frameRef.current?.contentWindow?.postMessage(
      { source: "table-tavern-host", kind: "chat", token: shellKey, chat, mvu },
      "*",
    );
  }, [chat, mvu, shellKey]);
  // 逐字稿變了就推（本樓沒變、不重掛的情形）；load 時再推一次，補 doc 建好到殼初始化之間漏掉的更新
  useEffect(() => {
    push();
  }, [push]);

  return (
    <div className="card-interface-overlay">
      {generatingName !== null && (
        <div className="card-interface-status" role="status">
          {t("typing", { name: generatingName })}
          <span className="typing">
            <i />
            <i />
            <i />
          </span>
        </div>
      )}
      <div className="card-interface-toolbar">
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
      {/* 單 iframe 直繪：key＝殼指紋，殼一換整支重掛（掛載時 srcdoc 就在，必然載入）。
          殼更新瞬間可能閃一下白，換來顯示的確定性。 */}
      {shellDoc !== null && (
        <iframe
          key={shellKey}
          ref={frameRef}
          onLoad={push}
          className="card-interface-frame"
          sandbox="allow-scripts"
          srcDoc={shellDoc}
          title={t("cardInterfaceOpen")}
        />
      )}
    </div>
  );
}
