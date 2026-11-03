// 選角（協定 §2）：名冊中可玩、未隱藏、沒保留給電腦的卡；或自建簡易角色（名字＋公開描述，執行端在桌上建卡後替他入座）。
import { useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";

export function SeatPicker({ view, act, desktop = false }: { view: RoomView; act: Act; desktop?: boolean }) {
  const { run, error } = useRun(act);
  const [name, setName] = useState(view.me.nickname);
  const [blurb, setBlurb] = useState("");
  const [busy, setBusy] = useState(false);
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
        <form
          className="seat-create"
          data-testid="seat-create"
          onSubmit={async (e) => {
            e.preventDefault();
            setBusy(true);
            await run("seat.create", { name: name.trim(), blurb: blurb.trim() });
            setBusy(false);
          }}
        >
          <h3>{t("seatCreate")}</h3>
          <label>
            {t("seatCreateName")}
            <input value={name} maxLength={40} onChange={(e) => setName(e.target.value)} />
          </label>
          <label>
            {t("seatCreateBlurb")}
            <textarea value={blurb} maxLength={1000} rows={3} onChange={(e) => setBlurb(e.target.value)} />
          </label>
          <button type="submit" disabled={busy || !name.trim()}>{t("seatCreate")}</button>
        </form>
      )}
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
