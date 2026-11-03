// 匯出成 VN 作品前的對話框（plans/vn-mode-pkg8.md §2.2）：個資提醒＋「附設定」勾選（預設不勾〔拍板 12〕；
// 卡片變數模式的桌停用並註明）。按匯出才收起對話框、接著另存與出包。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../../i18n";
import type { BibleView } from "../../../shared/contracts/backend-contracts";
import { Dialog } from "../../../shared/ui/Dialog";
import { exportVnWork } from "./vn-work";
import "./vn-work.css";

export function ExportWorkDialog({
  worldId,
  tableName,
  onClose,
  onError,
}: {
  worldId: string;
  tableName: string;
  onClose: () => void;
  onError: (reason: unknown) => void;
}) {
  const [view, setView] = useState<Pick<BibleView, "cover" | "vars_mode"> | null>(null);
  const [settings, setSettings] = useState(false);
  // 開窗時讀一次（封面有沒有、是不是變數模式）；onError 每次重畫都是新函式，不放進依賴
  useEffect(() => {
    invoke<BibleView>("vn_bible_read", { worldId }).then(setView, onError);
  }, [worldId]);
  const submit = () => {
    if (!view) return;
    onClose();
    exportVnWork(worldId, tableName, { settings: settings && !view.vars_mode, cover: view.cover }).catch(onError);
  };
  return (
    <Dialog
      title={t("vnWorkExport")}
      size="s"
      className="vn-work-dialog"
      onDismiss={onClose}
      end={
        <>
          <button type="button" className="btn" onClick={onClose}>
            {t("dialogCancel")}
          </button>
          <button type="button" className="btn btn-primary" disabled={!view} onClick={submit}>
            {t("vnWorkExport")}
          </button>
        </>
      }
    >
      <p>{t("vnWorkExportWarn")}</p>
      <label className="inline">
        <input
          type="checkbox"
          checked={settings && !view?.vars_mode}
          disabled={!view || view.vars_mode}
          onChange={(event) => setSettings(event.currentTarget.checked)}
        />
        {t("vnWorkSettings")}
      </label>
      {view?.vars_mode && <p className="vn-work-note">{t("vnWorkSettingsVars")}</p>}
    </Dialog>
  );
}
