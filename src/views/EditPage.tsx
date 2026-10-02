// 編輯頁共用頁框（角色卡／玩家卡／世界設定）：整面取代遊玩畫面，不是 modal——composer 不渲染＝編輯中無法發言。
// 頂列固定不隨內容捲走：左返回、標題、右 ⋯ 與主鈕「儲存」；按鈕位置不隨狀態位移，
// 未儲存數與存檔結果放頂列下方的狀態區，長譯文不會把頂列擠爆。
// 儲存鈕用 form 屬性送出外部表單：世界設定頁的條目表單是另一個 form，不能整頁包成一個巢狀。
import type { ReactNode } from "react";
import { t } from "../i18n";
import { backendText } from "../shared/ui/backend-text";
import { IconBack } from "../shared/ui/icons";
import { MoreMenu, type MoreMenuItem } from "../shared/ui/MoreMenu";

interface EditPageProps {
  title: string;
  onBack: () => void;
  /** 儲存鈕送出的表單 id；省略（載入中、載入失敗）＝儲存停用但占位 */
  formId?: string;
  /** ⋯ 的項目；空陣列就不渲染 ⋯，也不留空位 */
  moreItems?: MoreMenuItem[];
  unsavedCount?: number;
  /** 操作結果：已儲存是 status，其餘一律當錯誤 alert */
  message?: string;
  messageIsError?: boolean;
  children?: ReactNode;
}

export function EditPage({
  title,
  onBack,
  formId,
  moreItems = [],
  unsavedCount = 0,
  message = "",
  messageIsError = false,
  children,
}: EditPageProps) {
  const hasStatus = unsavedCount > 0 || message !== "";
  return (
    <div className="edit-page">
      <header className="edit-page-bar">
        <button type="button" className="btn btn-ghost edit-page-back" onClick={onBack}>
          <IconBack />
          {t("backToNow")}
        </button>
        <h2 className="edit-page-title" title={title}>
          {title}
        </h2>
        <span className="toolbar-spacer" />
        {moreItems.length > 0 && <MoreMenu label={t("moreActions")} items={moreItems} />}
        <button
          type="submit"
          form={formId}
          className="btn btn-primary btn-shrink edit-page-save"
          title={t("saveBtn")}
          disabled={formId === undefined}
        >
          <span className="btn-label">{t("saveBtn")}</span>
        </button>
      </header>
      {/* 沒東西就整塊不渲染：頁框的 gap 只落在實際存在的節點之間 */}
      {hasStatus && (
        // 限高內捲：可聚焦，超長錯誤才能只用鍵盤捲完
        <div className="edit-page-status" tabIndex={0}>
          {message && <span role={messageIsError ? "alert" : "status"}>{backendText(message)}</span>}
          {unsavedCount > 0 && (
            <span className="unsaved-hint" role="status">
              {t("unsavedChanges", { n: unsavedCount })}
            </span>
          )}
        </div>
      )}
      <div className="edit-page-body">{children}</div>
    </div>
  );
}
