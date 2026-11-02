// 選角（協定 §2）：名冊中可玩、未隱藏、未被認領的卡。自建角色要執行端在桌上建卡，v1 先只做選既有卡，入口停用並標註。
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";

export function SeatPicker({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  const cards = view.cards.filter((c) => c.playable && !c.hidden);
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
      <button type="button" className="ghost" disabled title={t("seatCreateSoon")}>{t("seatCreate")}</button>
      <div className="muted">{t("seatCreateSoon")}</div>
      {error && <div className="field-error">{error}</div>}
    </div>
  );
}
