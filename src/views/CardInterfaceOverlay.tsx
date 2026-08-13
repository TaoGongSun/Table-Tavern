// 卡片自帶介面的覆蓋層：打字狀態列、關閉鈕與那支沙盒 iframe。純 markup——
// 「要不要掛上去」的條件留在 App（只在遊玩畫面出現），殼內容與生成狀態各由自己的 controller 擁有。
import { t } from "../i18n";

interface CardInterfaceOverlayProps {
  /** 正在生成的那位要顯示的名字；null＝沒人在打字，狀態列不出現 */
  generatingName: string | null;
  /** 介面殼的 HTML；null＝還沒備好，不掛 iframe */
  shellDoc: string | null;
  /** 殼指紋：換一份殼就讓整支 iframe 重掛 */
  shellKey: string;
  onClose: () => void;
}

export function CardInterfaceOverlay({
  generatingName,
  shellDoc,
  shellKey,
  onClose,
}: CardInterfaceOverlayProps) {
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
          className="modal-close card-interface-close"
          aria-label={t("cardInterfaceClose")}
          onClick={() => onClose()}
        >
          ✕
        </button>
      </div>
      {/* 單 iframe 直繪：key＝殼指紋，殼一換整支重掛（掛載時 srcdoc 就在，必然載入）。
          殼更新瞬間可能閃一下白，換來顯示的確定性。 */}
      {shellDoc !== null && (
        <iframe
          key={shellKey}
          className="card-interface-frame"
          sandbox="allow-scripts"
          srcDoc={shellDoc}
          title={t("cardInterfaceOpen")}
        />
      )}
    </div>
  );
}
