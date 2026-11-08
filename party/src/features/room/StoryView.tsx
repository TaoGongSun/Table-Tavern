// 故事串：GM 旁白、NPC 對話、玩家行動、系統標記；串流中那段即時長出，定稿整段取代。
// 一律純文字（協定 §10：別人的行動與模型輸出都是不可信文字，不插 HTML）。
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { DicePoolView } from "@desktop/features/dice/DicePoolView";
import { streamDisplayText } from "@desktop/features/play/stream-display";
import type { Speaker } from "../../../server/core/types";
import { t, currentLang, type MsgKey } from "../../i18n";
import type { RoomView } from "./room-state";

const MARKER: Record<string, MsgKey> = {
  scene_summary: "markerSceneSummary",
  card_arrival: "markerCardArrival",
  person_arrival: "markerPersonArrival",
  state_update: "markerStateUpdate",
  round_undone: "markerRoundUndone",
  solo_before: "markerSoloBefore",
  solo_paused: "markerSoloPaused",
  room_reopened: "markerRoomReopened",
};

function Part({
  view,
  speaker,
  text,
  flag,
  dicePool,
}: {
  view: RoomView;
  speaker: Speaker;
  text: string;
  flag?: string;
  dicePool?: string;
}) {
  const name =
    speaker.kind === "gm"
      ? t("speakerGm")
      : speaker.kind === "system"
        ? t(MARKER[speaker.marker])
        : speaker.name;
  const color =
    "cardId" in speaker ? view.cards.find((c) => c.cardId === speaker.cardId)?.color : null;
  return (
    <div className={`part part-${speaker.kind}`} style={color ? { borderColor: color } : undefined}>
      <div className="part-name">{name}</div>
      <div className="part-text">{text}</div>
      {speaker.kind === "gm" && (
        <DicePoolView pool={dicePool} text={text} language={currentLang()} />
      )}
      {flag && <div className="part-flag">{flag}</div>}
    </div>
  );
}

/** `tail`：故事串最尾的動作（房主的收回／重生），跟著故事一起捲。
 *  `onReading`（手機）：離最底超過一屏回報 true、捲回最底回報 false；`jump` 每變一次就捲回最底；
 *  `onScrollMove`：每次故事區捲動都回報（打過字後，沒焦點的捲動才解除「維持展開」）；
 *  `folded`：上下列目前收起（展開時故事區變矮，貼著最底的要重新貼回去）。 */
export function StoryView({
  view,
  loadOlder,
  tail,
  onReading,
  onScrollMove,
  jump = 0,
  folded = false,
}: {
  view: RoomView;
  loadOlder: (beforeSeq: number) => Promise<number>;
  tail?: ReactNode;
  onReading?: (reading: boolean) => void;
  onScrollMove?: () => void;
  jump?: number;
  folded?: boolean;
}) {
  const box = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const reading = useRef(false);
  const report = (next: boolean) => {
    if (reading.current === next) return;
    reading.current = next;
    onReading?.(next);
  };
  useEffect(() => {
    if (!jump || !box.current) return;
    box.current.scrollTop = box.current.scrollHeight;
    stick.current = true;
    report(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jump]);
  // 換分頁回來是新掛上的故事區：從最底開始，不沿用收起狀態
  useEffect(() => () => onReading?.(false), [onReading]);
  // 收起的上下列一展開故事區就變矮：在捲動事件量到「離底一段」之前先貼回最底
  useLayoutEffect(() => {
    if (!folded && stick.current && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [folded]);
  // 故事區變矮（鍵盤彈出、視窗縮放）時，原本貼著最底就繼續貼著，新段落照樣自動跟上
  useEffect(() => {
    const el = box.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => {
      if (stick.current) el.scrollTop = el.scrollHeight;
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const [cursor, setCursor] = useState<number | null>(null); // null＝還沒翻過；0＝沒有更早的了
  const s = view.streaming;
  useEffect(() => {
    if (stick.current && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [view.story.length, s?.text, view.round]);
  const name = (cardId: string) => view.cards.find((c) => c.cardId === cardId)?.name ?? cardId;
  // 房主 GM 回合期間，之前已送出的行動留在 carry，照樣列著（GM 說完放回名單）
  const pending = (
    view.round?.phase === "acting" ? view.round.roster : (view.round?.carry ?? [])
  ).filter((e) => e.status === "submitted");
  return (
    <div
      className="story"
      ref={box}
      data-testid="story"
      onScroll={(e) => {
        onScrollMove?.();
        const el = e.currentTarget;
        const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
        stick.current = gap < 80;
        if (stick.current) report(false);
        else if (gap > el.clientHeight) report(true);
      }}
    >
      {cursor !== 0 &&
        view.room.scene > 0 && ( // 第一幕之前沒有更早的劇情
          <button
            type="button"
            className="ghost older"
            onClick={async () => setCursor(await loadOlder(cursor ?? view.seq + 1))}
          >
            {t("storyOlder")}
          </button>
        )}
      {view.story.map((p) =>
        p.handed && p.speaker.kind === "player" ? (
          // 交給 GM：旁白式一行（樣式比照系統行），左邊色條保留角色色
          <div
            key={`${p.round}:${p.part}`}
            className="part part-system part-handed"
            style={{
              borderColor:
                view.cards.find((c) => "cardId" in p.speaker && c.cardId === p.speaker.cardId)
                  ?.color ?? undefined,
            }}
          >
            {t("storyHandedToGm", { name: "name" in p.speaker ? p.speaker.name : "" })}
          </div>
        ) : (
          <Part
            key={`${p.round}:${p.part}`}
            view={view}
            speaker={p.speaker}
            text={p.text}
            dicePool={p.dicePool}
            flag={p.interrupted ? t("storyInterrupted") : undefined}
          />
        ),
      )}
      {s && (
        <div data-testid="streaming">
          <Part
            view={view}
            speaker={s.speaker}
            text={streamDisplayText(s.text)}
            flag={t("storyStreaming")}
          />
        </div>
      )}
      {pending.map((e) => (
        // 行動送出即公開（裁決 9）：結算前大家就看得到
        <div key={e.memberId} data-testid="submitted">
          <Part
            view={view}
            speaker={{
              kind: "player",
              memberId: e.memberId,
              cardId: e.cardId,
              name: name(e.cardId),
            }}
            text={e.text ?? ""}
            flag={t("storySubmitted")}
          />
        </div>
      ))}
      {tail}
    </div>
  );
}
