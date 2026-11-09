// 舞台：背景層、三個槽位的立繪（VN 卡給了身高就照比例縮、下限 0.8、下緣對齊；獨自一人站中間）、說話者亮其他壓暗、
// 特寫（以臉為錨放大）與震動、地點字卡（無背景圖時顯示在左上）、雨層（立繪之上，不跟著震動）。
// 退路鏈：該變體該表情臉層 → 該變體 neutral → 預設變體＋共用臉層（SpriteFigure）→ 角色卡原圖 → 剪影＋名牌。
import { cloneElement, useEffect, useRef, useState, type ReactElement } from "react";
import type { SpriteView } from "../../shared/contracts/backend-contracts";
import { SpriteFigure } from "./SpriteFigure";
import { RainLayer, StageBackdrop, spriteTint } from "./StageBackdrop";
import type { BackdropImage } from "./useBackgrounds";
import type { Stage as StageState } from "./vn-cells";

/** 身高比例下限：名單裡有特別高的角色時，一般人不至於縮成小人 */
export const MIN_SCALE = 0.8;
const CENTER_SLOT = 2;
/** 換立繪（卡圖／剪影→定稿、換定稿）的交叉淡入長度，與 CSS 的 vn-swap 一致 */
const SWAP_MS = 600;

/** 畫面上的位置：獨自一人在台上站中央槽，兩人以上照分配到的槽（換位走 left 過渡） */
export const placeOf = (slots: (string | null)[], slot: number) =>
  slots.filter(Boolean).length === 1 ? CENTER_SLOT : slot;

/** 立繪換新只在換格（推進、換說話者）或舞台被蓋住時套用，不在打字中途硬換 */
function useSettled(sprites: Record<string, SpriteView>, cellKey: string, hold: boolean) {
  const [settled, setSettled] = useState(sprites);
  const latest = useRef(sprites);
  latest.current = sprites;
  useEffect(() => setSettled(latest.current), [cellKey]);
  useEffect(() => {
    if (hold) setSettled(sprites);
  }, [hold, sprites]);
  return settled;
}

/** 同一槽位的圖換了身分（卡圖→定稿、換定稿）：舊的淡出、新的淡入（render 收額外的 class） */
function Swap({ id, render }: { id: string; render: (extra: string) => ReactElement }) {
  const last = useRef<{ id: string; node: ReactElement }>({ id, node: render(" vn-swap-out") });
  const [leaving, setLeaving] = useState<{ id: string; node: ReactElement } | null>(null);
  useEffect(() => {
    if (last.current.id === id) return;
    setLeaving(last.current);
    const timer = setTimeout(() => setLeaving(null), SWAP_MS);
    return () => clearTimeout(timer);
  }, [id]);
  useEffect(() => {
    last.current = { id, node: render(" vn-swap-out") };
  });
  // 換身分的這一輪 render 時 effect 還沒跑：直接拿上一個當淡出的
  const out = last.current.id !== id ? last.current : leaving;
  return (
    <>
      {out && cloneElement(out.node, { key: `out-${out.id}` })}
      {cloneElement(render(out ? " vn-swap-in" : ""), { key: id })}
    </>
  );
}

export function Stage({
  stage,
  sprites,
  images,
  backdrop,
  cellKey,
  backgroundState,
  onBackgroundRetry,
  showLocation = true,
  hold = false,
}: {
  stage: StageState;
  backgroundState?: string;
  onBackgroundRetry?: () => void;
  showLocation?: boolean;
  /** 角色 id → 立繪（定稿＋臉層） */
  sprites: Record<string, SpriteView>;
  /** 角色 id → 角色卡原圖：沒立繪時的佔位 */
  images: Record<string, string>;
  /** 目前背景鍵的圖；undefined＝還沒讀過（同沒有） */
  backdrop?: BackdropImage;
  /** 目前格的鍵：震動以它為 key，每格最多震一次 */
  cellKey: string;
  /** 標題、面板、字卡蓋著播放器：立繪淡出先暫停 */
  hold?: boolean;
}) {
  const closeup = stage.directive === "closeup" ? stage.speaker : null;
  const settled = useSettled(sprites, cellKey, hold);
  const tint = spriteTint(stage.backdrop?.period, stage.weather);
  return (
    <div className="vn-stage">
      {stage.backdrop && (
        <StageBackdrop image={backdrop ?? null} period={stage.backdrop.period} weather={stage.weather} location={showLocation ? stage.location ?? undefined : undefined} state={backgroundState} onRetry={onBackgroundRetry} />
      )}
      <div
        key={stage.directive === "shake" ? cellKey : undefined}
        className={`vn-cast${stage.directive === "shake" ? " vn-shake" : ""}`}
      >
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
            <figure
              key={id}
              className={`vn-sprite vn-slot-${placeOf(stage.slots, slot)} vn-${state}`}
              style={settled[id]?.scale ? { ["--vn-scale" as string]: Math.max(MIN_SCALE, settled[id].scale) } : undefined}
            >
              <Swap
                id={settled[id] ? `sprite-${settled[id].meta.revs.base ?? 0}` : images[id] ? "card" : "silhouette"}
                render={(extra) =>
                  settled[id] ? (
                    <SpriteFigure
                      sprite={settled[id]}
                      expression={stage.expressions[id] ?? "neutral"}
                      variant={stage.variants[id]}
                      alt={stage.names[id]}
                      className={`vn-layers${extra}`}
                      tint={tint}
                    />
                  ) : images[id] ? (
                    <img className={extra.trim() || undefined} src={images[id]} alt={stage.names[id]} />
                  ) : (
                    <div className={`vn-silhouette${extra}`} aria-label={stage.names[id]} />
                  )
                }
              />
            </figure>
          );
        })}
      </div>
      <RainLayer on={stage.weather === "rain"} period={stage.backdrop?.period} />
    </div>
  );
}
