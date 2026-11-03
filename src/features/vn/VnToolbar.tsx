// VN 播放器工具列（包 6 §2.8 重排）：回標題、桌名、停止、上一格、自動、快進、紀錄、存檔、下一章、聊天視圖、⋯。
// 旗標提示搬到標題畫面的設定；重生成背景、結局一覽、從某一章重玩、匯出成 VN 卡在 ⋯。
import { t } from "../../i18n";
import { IconHome, IconStop } from "../../shared/ui/icons";
import { MoreMenu, type MoreMenuItem } from "../../shared/ui/MoreMenu";

export function VnToolbar({
  tableName,
  busy,
  actionsDisabled,
  ended,
  canStop,
  onStop,
  auto,
  onAuto,
  skip,
  onSkip,
  canBack,
  onBack,
  onLog,
  onSaves,
  onNextChapter,
  onShowChat,
  onTitle,
  more,
}: {
  tableName: string;
  busy: boolean;
  actionsDisabled: boolean;
  ended: boolean;
  canStop: boolean;
  onStop: () => void;
  auto: boolean;
  onAuto: () => void;
  skip: boolean;
  onSkip: () => void;
  canBack: boolean;
  onBack: () => void;
  onLog: () => void;
  onSaves: () => void;
  onNextChapter: () => void;
  onShowChat: () => void;
  onTitle: () => void;
  more: MoreMenuItem[];
}) {
  return (
    <header className="vn-toolbar">
      <button
        type="button"
        className="btn btn-ghost btn-icon"
        aria-label={t("vnBackToTitle")}
        title={t("vnBackToTitle")}
        disabled={busy}
        onClick={onTitle}
      >
        <IconHome />
      </button>
      <h1 title={tableName}>{tableName}</h1>
      <span className="toolbar-spacer" />
      {canStop && (
        <button type="button" className="btn" onClick={onStop}>
          <IconStop />
          {t("stopResponse")}
        </button>
      )}
      <button type="button" className="btn btn-ghost" disabled={!canBack} onClick={onBack}>
        {t("vnBack")}
      </button>
      <button type="button" className="btn btn-ghost" aria-pressed={auto} onClick={onAuto}>
        {t("vnAuto")}
      </button>
      <button type="button" className="btn btn-ghost" aria-pressed={skip} onClick={onSkip}>
        {t("vnSkip")}
      </button>
      <button type="button" className="btn btn-ghost" onClick={onLog}>
        {t("vnLog")}
      </button>
      <button type="button" className="btn btn-ghost" disabled={busy} onClick={onSaves}>
        {t("vnSave")}
      </button>
      <button type="button" className="btn btn-ghost" disabled={actionsDisabled || ended} onClick={onNextChapter}>
        {t("vnNextChapter")}
      </button>
      <button type="button" className="btn btn-ghost" onClick={onShowChat}>
        {t("vnShowChat")}
      </button>
      <MoreMenu label={t("moreActions")} disabled={busy} items={more} />
    </header>
  );
}
