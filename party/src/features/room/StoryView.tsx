// 故事串：GM 旁白、NPC 對話、玩家行動、系統標記；串流中那段即時長出，定稿整段取代。
// 一律純文字（協定 §10：別人的行動與模型輸出都是不可信文字，不插 HTML）。
import { useEffect, useRef, useState, type ReactNode } from "react";
import { streamDisplayText } from "@desktop/features/play/stream-display";
import type { Speaker } from "../../../server/core/types";
import { t, type MsgKey } from "../../i18n";
import type { RoomView } from "./room-state";

const MARKER: Record<string, MsgKey> = { scene_summary: "markerSceneSummary", card_arrival: "markerCardArrival", person_arrival: "markerPersonArrival", state_update: "markerStateUpdate", round_undone: "markerRoundUndone", solo_before: "markerSoloBefore", solo_paused: "markerSoloPaused", room_reopened: "markerRoomReopened" };

function Part({ view, speaker, text, flag }: { view: RoomView; speaker: Speaker; text: string; flag?: string }) {
  const name = speaker.kind === "gm" ? t("speakerGm") : speaker.kind === "system" ? t(MARKER[speaker.marker]) : speaker.name;
  const color = "cardId" in speaker ? view.cards.find((c) => c.cardId === speaker.cardId)?.color : null;
  return (
    <div className={`part part-${speaker.kind}`} style={color ? { borderColor: color } : undefined}>
      <div className="part-name">{name}</div>
      <div className="part-text">{text}</div>
      {flag && <div className="part-flag">{flag}</div>}
    </div>
  );
}

/** `tail`：故事串最尾的動作（房主的收回／重生），跟著故事一起捲 */
export function StoryView({ view, loadOlder, tail }: { view: RoomView; loadOlder: (beforeSeq: number) => Promise<number>; tail?: ReactNode }) {
  const box = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const [cursor, setCursor] = useState<number | null>(null); // null＝還沒翻過；0＝沒有更早的了
  const s = view.streaming;
  useEffect(() => {
    if (stick.current && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [view.story.length, s?.text, view.round]);
  const name = (cardId: string) => view.cards.find((c) => c.cardId === cardId)?.name ?? cardId;
  const pending = view.round?.phase === "acting" ? view.round.roster.filter((e) => e.status === "submitted") : [];
  return (
    <div
      className="story"
      ref={box}
      data-testid="story"
      onScroll={(e) => (stick.current = e.currentTarget.scrollHeight - e.currentTarget.scrollTop - e.currentTarget.clientHeight < 80)}
    >
      {cursor !== 0 && view.room.scene > 0 && ( // 第一幕之前沒有更早的劇情
        <button type="button" className="ghost older" onClick={async () => setCursor(await loadOlder(cursor ?? view.seq + 1))}>
          {t("storyOlder")}
        </button>
      )}
      {view.story.map((p) => (
        <Part key={`${p.round}:${p.part}`} view={view} speaker={p.speaker} text={p.text} flag={p.interrupted ? t("storyInterrupted") : undefined} />
      ))}
      {s && (
        <div data-testid="streaming">
          <Part view={view} speaker={s.speaker} text={streamDisplayText(s.text)} flag={t("storyStreaming")} />
        </div>
      )}
      {pending.map((e) => (
        // 行動送出即公開（裁決 9）：結算前大家就看得到
        <div key={e.memberId} data-testid="submitted">
          <Part view={view} speaker={{ kind: "player", memberId: e.memberId, cardId: e.cardId, name: name(e.cardId) }} text={e.text ?? ""} flag={t("storySubmitted")} />
        </div>
      ))}
      {tail}
    </div>
  );
}
