import { t } from "../../i18n";
import type { KnownMarker, TranscriptEvent } from "../../shared/contracts/backend-contracts";

const NAMED: Record<string, "name" | "title"> = {
  card_arrival: "name",
  card_private: "name",
  gm_call: "name",
  person_arrival: "title",
  player_watch: "name",
  player_handed_to_gm: "name",
  player_arrival: "name",
  player_to_npc: "name",
  player_absent: "name",
  player_delegated: "name",
  player_returned: "name",
};
const BARE = new Set(["scene_summary", "state_update", "dice_roll"]);

/** 執行期形狀檢查：已知 type 且必要欄位是字串才算數；未知、畸形（寬鬆讀取原樣帶來的）一律回 null。 */
export function parseMarker(value: unknown): KnownMarker | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  const type = record.type;
  if (typeof type !== "string") return null;
  if (BARE.has(type)) return { type } as KnownMarker;
  const field = NAMED[type];
  if (field === undefined || typeof record[field] !== "string") return null;
  return { type, [field]: record[field] } as KnownMarker;
}

/** 標頭第一行（照目前介面語系）。點名玩家而玩家沒名字時退回玩家稱呼。 */
export function markerHeading(marker: KnownMarker): string {
  switch (marker.type) {
    case "scene_summary":
      return t("marker_scene_summary");
    case "card_arrival":
      return t("marker_card_arrival", { name: marker.name });
    case "person_arrival":
      return t("marker_person_arrival", { title: marker.title });
    case "card_private":
      return t("marker_card_private", { name: marker.name });
    case "dice_roll":
      return t("marker_dice_roll");
    case "state_update":
      return t("marker_state_update");
    case "gm_call":
      return t("marker_gm_call", { name: marker.name.trim() || t("playerLabel") });
    // 多人房間（multiplayer-room 包 3）的在場標記：本文都是空的，整則只有標頭
    case "player_watch":
      return t("marker_player_watch", { name: marker.name });
    // 交給 GM（room-speak-request §2）：標記原文是寫給 GM 的演法提示；玩家看到的標頭只寫狀態
    case "player_handed_to_gm":
      return t("roomMarkerHandedToGm", { name: marker.name });
    case "player_arrival":
      return t("marker_player_arrival", { name: marker.name });
    case "player_to_npc":
      return t("marker_player_to_npc", { name: marker.name });
    case "player_absent":
      return t("marker_player_absent", { name: marker.name });
    // AI 代管（room-seat-delegate）：暫離與接回
    // 標記原文（marker_player_delegated）也是寫給 GM 的演法提示；玩家看到的標頭只寫狀態
    case "player_delegated":
      return t("roomMarkerDelegated", { name: marker.name });
    case "player_returned":
      return t("marker_player_returned", { name: marker.name });
  }
}

/** 畫面上的事件全文：標頭＋段標＋本文（與後端 event_full_text 同規則）；沒有可用代碼就是本文。 */
export function eventDisplayText(event: TranscriptEvent): string {
  const marker = parseMarker(event.marker);
  if (marker === null) return event.text;
  const heading = markerHeading(marker);
  const text = event.text;
  switch (marker.type) {
    case "card_arrival":
      return text.trim() ? `${heading}\n${t("marker_public_profile")}\n${text}` : heading;
    case "card_private":
      return `${heading}\n${t("marker_private_profile")}\n${text}`;
    case "gm_call":
      return heading;
    default:
      return text ? `${heading}\n${text}` : heading;
  }
}

/** 名牌：沒有名字的玩家發言退回玩家稱呼。 */
export function speakerDisplayName(event: TranscriptEvent): string {
  return event.kind === "player" && !event.speaker_name.trim() ? t("playerLabel") : event.speaker_name;
}
