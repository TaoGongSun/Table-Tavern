// 附設定的作品包匯入時二選一（plans/vn-mode-pkg8.md §2.3）：當作品玩（重播桌，不需要 AI），或當可續寫的桌
// （需要 AI，可從任一章或選項點接著寫；劇透鎖照玩家勾選）。取消不建桌。
import { useState } from "react";
import { t } from "../../../i18n";
import type { VnWorkProbe } from "../../../shared/contracts/backend-contracts";
import { Dialog } from "../../../shared/ui/Dialog";
import { type VnWorkMode, vnWorkConfirmText } from "./vn-work";
import "./vn-work.css";

export function ImportWorkDialog({
  probe,
  onChoose,
  onCancel,
}: {
  probe: VnWorkProbe;
  onChoose: (mode: VnWorkMode, spoilerLock: boolean) => void;
  onCancel: () => void;
}) {
  const [spoilerLock, setSpoilerLock] = useState(true);
  return (
    <Dialog
      title={t("vnWorkConfirmTitle")}
      size="s"
      className="vn-work-dialog"
      onDismiss={onCancel}
      start={
        <button type="button" className="btn" onClick={onCancel}>
          {t("dialogCancel")}
        </button>
      }
      end={
        <button type="button" className="btn btn-primary" onClick={() => onChoose("replay", false)}>
          {t("vnWorkAsReplay")}
        </button>
      }
    >
      <p>{vnWorkConfirmText(probe, true)}</p>
      {/* 劇透鎖只管可續寫的桌：跟那顆鈕放在一起 */}
      <div className="vn-work-continue">
        <button type="button" className="btn" onClick={() => onChoose("continue", spoilerLock)}>
          {t("vnWorkAsContinue")}
        </button>
        <label className="inline">
          <input
            type="checkbox"
            checked={spoilerLock}
            onChange={(event) => setSpoilerLock(event.currentTarget.checked)}
          />
          {t("vnSpoilerCheckbox")}
        </label>
      </div>
    </Dialog>
  );
}
