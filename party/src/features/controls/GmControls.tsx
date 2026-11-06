// 房主的 GM 鈕（room-host-actions §2、room-desktop-layout §2-7）：分段鈕，主鈕是上次用的動作（預設 GM 旁白）、按下直接執行；
// ▾ 選單列出旁白、推進、讓每張 NPC 說一句，點了直接執行並成為新的主鈕。「讓 NPC X 接話」（GM 點名）另一顆主按鈕。
// 照單人版放在行動格的動作列左側；這一回合已送出的行動保留到 GM 說完（room-speak-request §3）。行動中且 GM 主機在線才能按，否則主鈕與選單一起停用並寫原因。
import { useState } from "react";
import { t } from "../../i18n";
import type { RoomView } from "../room/room-state";
import { useRun, type Act } from "../room/useRoom";
import { PartyMenu } from "./PartyMenu";

type Move = { mode: "narrate" } | { mode: "advance" } | { mode: "call"; cardId: string };

/** GM 鈕停用的原因；null＝可以按 */
const gmBlocked = (view: RoomView) =>
  !view.executor.online
    ? t("startNeedExecutor")
    : view.executor.syncing
      ? t("roundHostSyncing")
      : view.round?.phase !== "acting"
        ? t("gmBusy")
        : null;
/** 這回合已送出（含交給 GM）的人數：按 GM 鈕時這些行動會保留 */
const keptCount = (view: RoomView) =>
  view.round?.roster.filter((e) => e.status === "submitted" || e.status === "skipped").length ?? 0;

/** GM 鈕列正下方的一行小字（手機沒有 hover，不能只寫在 title）：有人已送出且 GM 鈕能按時才出現 */
export function GmKeepNote({ view }: { view: RoomView }) {
  const kept = keptCount(view);
  if (!kept || gmBlocked(view) !== null) return null;
  return (
    <div className="action-hint gm-keep-note" data-testid="gm-keep-note">
      {t("gmKeepLine", { count: kept })}
    </div>
  );
}

export function GmControls({ view, act }: { view: RoomView; act: Act }) {
  const { run, error } = useRun(act);
  const [last, setLast] = useState<Move>({ mode: "narrate" }); // 只記在記憶體
  const r = view.round;
  const npcs = view.cards.filter(
    (c) => c.playable && !c.hidden && c.claimedBy === null && !c.reserved,
  );
  const blocked = gmBlocked(view);
  const called = r?.called ? npcs.find((c) => c.cardId === r.called) : undefined;
  // 離座後交給 GM 演的玩家卡：備註放在整個項目最後，免得看成在場的人（只記這次連線期間離座的）
  const npc = (key: "requestReplyItem" | "callNominated", cardId: string) =>
    t(key, { name: npcs.find((c) => c.cardId === cardId)?.name ?? "" }) +
    (view.departed.includes(cardId) ? t("npcDeparted") : "");
  const kept = keptCount(view);
  const keepNote = kept ? t("gmKeepCount", { count: kept }) : t("gmNoResolveNote");
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
        : npc("requestReplyItem", m.cardId);
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
          title={blocked ?? `${t("callNominatedHint", { name: called.name })}（${keepNote}）`}
          onClick={() => go({ mode: "call", cardId: called.cardId })}
          data-testid="gm-call"
        >
          {npc("callNominated", called.cardId)}
        </button>
      )}
      <span className="gm-seg" data-testid="gm-controls">
        <button
          type="button"
          disabled={blocked !== null}
          title={blocked ?? `${note(main)}（${keepNote}）`}
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
