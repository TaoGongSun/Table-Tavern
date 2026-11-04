// 房主的 GM 鈕（room-host-actions §2、room-desktop-layout §2-7）：分段鈕，主鈕是上次用的動作（預設 GM 旁白）、按下直接執行；
// ▾ 選單列出旁白、推進、請每張 NPC 發言，點了直接執行並成為新的主鈕。「讓 X 接話」（GM 點名）另一顆主按鈕。
// 照單人版放在行動格的動作列左側；這一回合已送出的行動會退回草稿。行動中且 GM 主機在線才能按，否則主鈕與選單一起停用並寫原因。
import { useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { PartyMenu } from "./PartyMenu";

type Move = { mode: "narrate" } | { mode: "advance" } | { mode: "call"; cardId: string };

export function GmControls({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  const [last, setLast] = useState<Move>({ mode: "narrate" }); // 只記在記憶體
  const r = view.round;
  const npcs = view.cards.filter(
    (c) => c.playable && !c.hidden && c.claimedBy === null && !c.reserved,
  );
  const blocked = !view.executor.online
    ? t("startNeedExecutor")
    : view.executor.syncing
      ? t("roundHostSyncing")
      : r?.phase !== "acting"
        ? t("gmBusy")
        : null;
  const called = r?.called ? npcs.find((c) => c.cardId === r.called) : undefined;
  // 上次請的那張 NPC 已經不能選（被認走、藏起來）：退回 GM 旁白
  const main: Move =
    last.mode === "call" && !npcs.some((c) => c.cardId === last.cardId)
      ? { mode: "narrate" }
      : last;
  const label = (m: Move) =>
    m.mode === "narrate"
      ? t("gmNarrate")
      : m.mode === "advance"
        ? t("gmAdvance")
        : t("requestReplyItem", { name: npcs.find((c) => c.cardId === m.cardId)?.name ?? "" });
  const note = (m: Move) =>
    t(
      m.mode === "narrate"
        ? "gmNarrateNote"
        : m.mode === "advance"
          ? "gmAdvanceNote"
          : "requestReplyHint",
    );
  const go = (m: Move) => {
    setLast(m);
    void run("host.gm", {
      round: r?.round ?? 0,
      mode: m.mode,
      ...(m.mode === "call" ? { cardId: m.cardId } : {}),
    });
  };
  const moves: Move[] = [
    { mode: "narrate" },
    { mode: "advance" },
    ...npcs.map((c) => ({ mode: "call" as const, cardId: c.cardId })),
  ];
  return (
    <>
      {called && (
        <button
          type="button"
          className="primary"
          disabled={blocked !== null}
          title={blocked ?? t("callNominatedHint", { name: called.name })}
          onClick={() => go({ mode: "call", cardId: called.cardId })}
          data-testid="gm-call"
        >
          {t("callNominated", { name: called.name })}
        </button>
      )}
      <span className="gm-seg" data-testid="gm-controls">
        <button
          type="button"
          disabled={blocked !== null}
          title={blocked ?? `${note(main)}（${t("gmNoResolveNote")}）`}
          onClick={() => go(main)}
          data-testid="gm-main"
        >
          {label(main)}
        </button>
        <PartyMenu
          label={t("gmMenu")}
          className="gm-seg-more"
          trigger="▾"
          disabled={blocked !== null}
          items={moves.map((m) => ({
            key: m.mode === "call" ? m.cardId : m.mode,
            label: label(m),
            note: note(m),
            onSelect: () => go(m),
          }))}
        />
      </span>
      {(!view.executor.online || view.executor.syncing) && (
        <span className="action-hint">{blocked}</span>
      )}
      {error && (
        <span className="field-error" data-testid="gm-error">
          {error}
        </span>
      )}
    </>
  );
}
