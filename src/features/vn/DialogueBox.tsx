// 對話框：名牌、打字機文字、點擊推進提示（▼）；停在幕末等下一輪時右下角換成「繼續」小鈕。
// 本身是 role="button" 的 div（裡面要放真按鈕）；Space／Enter 由播放器的鍵盤處理。
import { useEffect, useRef } from "react";
import { t } from "../../i18n";
import type { FlatCell } from "./vn-cells";
import { charLen, sliceChars } from "./vn-cells";

export function DialogueBox({
  cell,
  shown,
  waiting,
  ready,
  onAdvance,
  onContinue,
}: {
  cell: FlatCell | undefined;
  shown: number;
  /** 生成中且已到結尾：顯示等待格 */
  waiting: boolean;
  /** 本格打完、可推進：顯示提示 */
  ready: boolean;
  onAdvance: () => void;
  /** 幕末的「繼續」（生成下一輪或重播桌換下一幕）；給了就取代 ▼ */
  onContinue?: { disabled: boolean; run: () => void };
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
    <div role="button" tabIndex={0} className="vn-dialogue" onClick={onAdvance}>
      {speaker && <span className="vn-nameplate">{speaker}</span>}
      <span ref={textRef} className={`vn-text${thought ? " vn-thought" : ""}`}>
        {thought && "（"}
        {sliceChars(text, shown)}
        {thought && shown >= charLen(text) && "）"}
        {waiting && <span className="vn-waiting"> …</span>}
      </span>
      {onContinue ? (
        <button
          type="button"
          className="vn-continue"
          disabled={onContinue.disabled}
          onClick={(event) => {
            event.stopPropagation();
            onContinue.run();
          }}
        >
          {t("vnContinue")}
        </button>
      ) : (
        ready && !waiting && <span className="vn-ctc" aria-hidden="true" />
      )}
    </div>
  );
}
