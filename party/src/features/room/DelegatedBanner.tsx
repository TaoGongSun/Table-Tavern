// 代管中的本人：回合列下方常駐一條，切到哪一頁都在；「接回」何時生效照當下狀態只寫會發生的那一種。
import { t } from "../../i18n";
import { DelegateButton } from "./DelegateButton";
import { allDelegated } from "./delegation";
import type { RoomView } from "./room-state";
import type { Act } from "./useRoom";

export function DelegatedBanner({ view, act }: { view: RoomView; act: Act }) {
  if (!view.me.delegated || !view.me.cardId || view.room.status === "closed") return null;
  // 房間正停著等人（行動中、名單空）：接回馬上進這回合名單
  const now = view.round?.phase === "acting" && view.round.roster.length === 0 && allDelegated(view);
  return (
    <div className="delegated-banner" data-testid="delegated-banner">
      <div className="grow">
        <div>{t("delegatedBanner")}</div>
        <div className="muted">{t(now ? "delegatedBackNow" : "delegatedBackNext")}</div>
      </div>
      <DelegateButton view={view} act={act} memberIds={[view.me.memberId]} on={false} label={t("delegateTakeBack")} className="primary" testId="delegate-take-back" />
    </div>
  );
}
