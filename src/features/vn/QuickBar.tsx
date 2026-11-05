// 快捷列（Ren'Py quick menu）：對話框正下方靠右一行小字。上一格｜自動｜快進｜紀錄｜存檔｜設定｜選單；生成中最左多「停止」。
// 滑鼠按下不搶焦點：Windows 點過的按鈕會留著焦點，之後按 Space 會再按它一次而不是推進。
import { t } from "../../i18n";

export function QuickBar({
  canStop,
  onStop,
  canBack,
  onBack,
  auto,
  onAuto,
  skip,
  onSkip,
  onLog,
  busy,
  onSaves,
  onSettings,
  onMenu,
}: {
  canStop: boolean;
  onStop: () => void;
  canBack: boolean;
  onBack: () => void;
  auto: boolean;
  onAuto: () => void;
  skip: boolean;
  onSkip: () => void;
  onLog: () => void;
  busy: boolean;
  onSaves: () => void;
  onSettings: () => void;
  onMenu: () => void;
}) {
  return (
    <nav className="vn-quick" onMouseDown={(event) => event.preventDefault()}>
      {canStop && (
        <button type="button" className="vn-quick-stop" onClick={onStop}>
          {t("stopResponse")}
        </button>
      )}
      <button type="button" disabled={!canBack} onClick={onBack}>
        {t("vnBack")}
      </button>
      <button type="button" aria-pressed={auto} onClick={onAuto}>
        {t("vnAuto")}
      </button>
      <button type="button" aria-pressed={skip} onClick={onSkip}>
        {t("vnSkip")}
      </button>
      <button type="button" onClick={onLog}>
        {t("vnLog")}
      </button>
      <button type="button" disabled={busy} onClick={onSaves}>
        {t("vnSave")}
      </button>
      <button type="button" onClick={onSettings}>
        {t("vnTitleSettings")}
      </button>
      <button type="button" onClick={onMenu}>
        {t("vnMenu")}
      </button>
    </nav>
  );
}
