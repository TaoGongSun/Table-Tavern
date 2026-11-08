// 名單頁頂端的常駐卡片（房主）：有人說某個離線座位是他的，兩列的狀態擺在一起讓房主判斷。不彈窗、不打斷回合。
import { t } from "../../i18n";
import { cardName } from "../room/delegation";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";

export function SeatRequests({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  if (!view.seatRequests.length) return null;
  const member = (id: string) => view.members.find((m) => m.memberId === id);
  return (
    <>
      {view.seatRequests.map((r) => {
        const asker = member(r.memberId);
        const old = member(r.occupantId);
        if (!asker || !old) return null;
        const answer = (accept: boolean) =>
          run("seat.request.resolve", { memberId: r.memberId, requestId: r.requestId, accept });
        const away =
          r.offlineSince === null
            ? t("hostClaimOfflineUnknown")
            : t("hostClaimOfflineFor", {
                n: Math.max(1, Math.round((Date.now() - r.offlineSince) / 60_000)),
              });
        return (
          <div
            key={r.requestId}
            className="seat-request"
            data-testid={`seat-request-${r.memberId}`}
          >
            <strong>
              {t("hostClaimTitle", { name: asker.nickname, card: cardName(view, r.cardId) })}
            </strong>
            <div className="muted">
              {t("hostClaimNewRow", {
                name: asker.nickname,
                state: t(asker.connected ? "online" : "offline"),
              })}
            </div>
            <div className="muted">
              {t("hostClaimOldRow", {
                name: old.nickname,
                card: cardName(view, r.cardId),
                state: away,
              })}
            </div>
            {asker.nickname === old.nickname && (
              <div className="muted">{t("hostClaimSameNick")}</div>
            )}
            <div className="panel-actions">
              <button type="button" className="primary" onClick={() => void answer(true)}>
                {t("hostClaimYes")}
              </button>
              <button type="button" onClick={() => void answer(false)}>
                {t("hostClaimNo")}
              </button>
            </div>
          </div>
        );
      })}
      {error && <div className="field-error">{error}</div>}
    </>
  );
}
