// 對話框：名牌、打字機文字、點擊推進提示；本身是按鈕，Space／Enter 由播放器的鍵盤處理（不走原生 click，免得推兩次）。
import { useEffect, useRef } from "react";
import type { FlatCell } from "./vn-cells";
import { sliceChars } from "./vn-cells";

export function DialogueBox({
  cell,
  shown,
  waiting,
  ready,
  onAdvance,
}: {
  cell: FlatCell | undefined;
  shown: number;
  /** 生成中且已到結尾：顯示等待格 */
  waiting: boolean;
  /** 本格打完、可推進：顯示提示 */
  ready: boolean;
  onAdvance: () => void;
}) {
  const content = cell?.cell;
  const speaker = content?.type === "dialogue" ? content.speaker_name : null;
  const text = content?.type === "narration" || content?.type === "dialogue" ? content.text : "";
  const thought = content?.type === "dialogue" && content.thought;
  const textRef = useRef<HTMLSpanElement>(null);
  // 超出框高時跟著打字機捲到最新一行
  useEffect(() => {
    const el = textRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [shown, cell?.key]);
  return (
    <button type="button" className="vn-dialogue" onClick={onAdvance}>
      {speaker && <span className="vn-nameplate">{speaker}</span>}
      <span ref={textRef} className={`vn-text${thought ? " vn-thought" : ""}`}>
        {sliceChars(text, shown)}
        {waiting && <span className="vn-waiting"> …</span>}
      </span>
      {ready && !waiting && <span className="vn-ctc" aria-hidden="true" />}
    </button>
  );
}
