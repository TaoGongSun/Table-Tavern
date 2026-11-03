// 舞台：三個槽位的立繪（角色卡原圖當佔位，沒圖＝剪影）、說話者亮其他壓暗、特寫與震動、地點字卡。
import type { Stage as StageState } from "./vn-cells";

export function Stage({
  stage,
  images,
  cellKey,
}: {
  stage: StageState;
  images: Record<string, string>;
  /** 目前格的鍵：震動以它為 key，每格最多震一次 */
  cellKey: string;
}) {
  const closeup = stage.directive === "closeup" ? stage.speaker : null;
  return (
    <div
      key={stage.directive === "shake" ? cellKey : undefined}
      className={`vn-stage${stage.directive === "shake" ? " vn-shake" : ""}`}
    >
      {stage.location && (
        <div key={stage.location} className="vn-location">
          {stage.time ? `${stage.location}｜${stage.time}` : stage.location}
        </div>
      )}
      {stage.slots.map((id, slot) => {
        if (!id) return null;
        const state = closeup
          ? closeup === id
            ? "closeup"
            : "faded"
          : stage.speaker === null
            ? "idle"
            : stage.speaker === id
              ? "speaking"
              : "dim";
        return (
          <figure key={id} className={`vn-sprite vn-slot-${slot} vn-${state}`}>
            {images[id] ? (
              <img src={images[id]} alt={stage.names[id]} />
            ) : (
              <div className="vn-silhouette" aria-label={stage.names[id]} />
            )}
          </figure>
        );
      })}
    </div>
  );
}
