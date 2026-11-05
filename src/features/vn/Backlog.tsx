// log：游標以前的正式格（名牌＋文字、thought 淡藍加（）、`[bg]` 一行地點、玩家選的選項）。打開停在最新一句；Esc 由播放器的鍵盤處理關。
import { useLayoutEffect, useRef } from "react";
import { t } from "../../i18n";
import { IconClose } from "../../shared/ui/icons";
import type { FlatCell } from "./vn-cells";

export function Backlog({ cells, onClose }: { cells: FlatCell[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (ref.current) ref.current.scrollTop = ref.current.scrollHeight;
  }, []);
  return (
    <div ref={ref} className="vn-backlog" role="dialog" aria-label={t("vnLog")}>
      <button
        type="button"
        className="btn btn-ghost btn-icon vn-backlog-close"
        aria-label={t("closeBtn")}
        title={t("closeBtn")}
        onClick={onClose}
      >
        <IconClose />
      </button>
      <ol>
        {cells.map(({ key, kind, cell }) => {
          if (kind === "pending") return null;
          if (kind === "player") return <li key={key}>▶ {cell.type === "narration" && cell.text}</li>;
          if (cell.type === "bg") return <li key={key} className="vn-backlog-bg">{cell.location}</li>;
          if (cell.type === "narration") return <li key={key}>{cell.text}</li>;
          if (cell.type !== "dialogue") return null;
          return (
            <li key={key}>
              <b>{cell.speaker_name}</b>
              <span className={cell.thought ? "vn-thought" : undefined}>{cell.thought ? `（${cell.text}）` : cell.text}</span>
            </li>
          );
        })}
      </ol>
    </div>
  );
}
