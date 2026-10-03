import { t } from "../../i18n";
import type { KnownMarker, TranscriptEvent } from "../../shared/contracts/backend-contracts";

const NAMED: Record<string, "name" | "title"> = {
  card_arrival: "name",
  card_private: "name",
  gm_call: "name",
  person_arrival: "title",
};
const BARE = new Set(["scene_summary", "state_update"]);

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
    case "state_update":
      return t("marker_state_update");
    case "gm_call":
      return t("marker_gm_call", { name: marker.name.trim() || t("playerLabel") });
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
