// 逐字稿事件標頭（後端 EventMarker 代碼）的十語系字典；與後端 data/scene/marker.rs 共讀同一份 JSON。
// 鍵是 `marker_<code>`，index.ts 的 t() 把這份併進同一個 MsgKey 空間；組字在 features/play/event-text.ts。
import COPY from "../../shared/contracts/transcript-marker.json";

type TranscriptMarkerLang = keyof typeof COPY;
type MarkerCode = keyof (typeof COPY)["zh-TW"];

export type TranscriptMarkerMsgKey = `marker_${MarkerCode}`;
export const TRANSCRIPT_MARKER_MESSAGE_KEYS = (Object.keys(COPY["zh-TW"]) as MarkerCode[]).map(
  (code) => `marker_${code}` as TranscriptMarkerMsgKey,
);

export function isTranscriptMarkerMsgKey(key: string): key is TranscriptMarkerMsgKey {
  // 與其他補充字典一樣容忍非字串鍵（既有呼叫端可能傳 undefined 進 t()）
  return typeof key === "string" && key.startsWith("marker_") && key.slice("marker_".length) in COPY["zh-TW"];
}

export function transcriptMarkerMessage(lang: TranscriptMarkerLang, key: TranscriptMarkerMsgKey): string {
  return COPY[lang][key.slice("marker_".length) as MarkerCode];
}
