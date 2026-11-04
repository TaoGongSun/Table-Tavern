// 選角（協定 §2）：名冊中可玩、未隱藏、沒保留給電腦的卡；或匯入角色卡、建立角色（執行端在桌上建卡後替他入座，room-player-cards）。
import { useState } from "react";
import { t } from "../../i18n";
import type { Upload } from "../my-card/card-upload";
import { errorText } from "../room/error-text";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";

interface Props {
  view: RoomView;
  act: Act;
  desktop?: boolean;
  /** null＝桌面版：不給匯入 */
  upload: Upload | null;
  onCreate: () => void;
  /** 匯入成功（入座後畫面換掉，提示由上層顯示） */
  onImported?: () => void;
}

export function SeatPicker({ view, act, desktop = false, upload, onCreate, onImported }: Props) {
  const { run, error, setError } = useRun(act);
  const [importing, setImporting] = useState(false);
  async function importCard(file: File | undefined) {
    if (!file || !upload) return;
    setImporting(true);
    const r = await upload("card", file);
    setImporting(false);
    setError(r.ok ? null : errorText(r.code ?? "E_INTERNAL", r.retryAfterMs));
    if (r.ok) onImported?.();
  }
  // 保留給電腦（房主自己的玩家卡）的卡：手機上不列，認走之後照常顯示「已有人」
  const cards = view.cards.filter((c) => c.playable && !c.hidden && (desktop || !c.reserved || c.claimedBy !== null));
  return (
    <div className="seat-picker" data-testid="seat-picker">
      <h2>{t("seatTitle")}</h2>
      {cards.map((c) => (
        <div key={c.cardId} className="seat-card">
          {c.avatar && <img src={c.avatar} alt="" />}
          <div className="seat-info">
            <strong>{c.name}</strong>
            <span className="muted">{c.blurb}</span>
          </div>
          <button
            type="button"
            disabled={c.claimedBy !== null}
            onClick={() => run("seat.claim", { cardId: c.cardId })}
          >
            {c.claimedBy ? t("seatTaken") : t("seatClaim")}
          </button>
        </div>
      ))}
      {view.room.settings.allowCustomCards && (
        <div className="panel-actions seat-entries" data-testid="seat-create">
          {/* 匯入：房主電腦建出只有角色本身的卡後自動入座（桌面版有自己的匯入，不給） */}
          {upload && (
            <label className="button-like">
              {importing ? t("seatImporting") : t("seatImport")}
              <input type="file" accept=".png,.json,image/png,application/json" hidden disabled={importing} onChange={(e) => (void importCard(e.target.files?.[0]), (e.currentTarget.value = ""))} />
            </label>
          )}
          <button type="button" onClick={onCreate}>{t("seatCreate")}</button>
        </div>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
