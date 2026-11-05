// 舞台背景層：目前背景的圖好了就淡入；還沒好＝前一張模糊頂著，第一張都還沒有＝黑幕（字卡由 Stage 疊在上面）。
// 圖是同地點基底（實際時段與要的不同）就疊該時段的色調遮罩（plans/vn-setup-wizard.md §3）：蓋背景（立繪只在夜晚壓半強度，見 Stage），
// 換時段時遮罩漸變；卡自訂時段沒有對應色就不套。
import { useEffect, useState } from "react";
import type { BackdropImage } from "./useBackgrounds";

/** 時段色調：multiply 疊色（白＝不變），opacity 控制強度 */
const TINTS: Record<string, { color: string; opacity: number }> = {
  morning: { color: "#ccd7f0", opacity: 0.75 },
  dusk: { color: "#ff9a5c", opacity: 0.75 },
  night: { color: "#22305f", opacity: 0.88 },
};

export function tintOf(period: string, image: BackdropImage | null) {
  if (!image?.period || image.period === period) return null;
  return TINTS[period] ?? null;
}

/** 夜晚立繪的色調：背景遮罩的半強度（其他時段不壓立繪） */
export function spriteNight(period: string | undefined): { color: string; opacity: number } | null {
  return period === "night" ? { color: TINTS.night.color, opacity: TINTS.night.opacity / 2 } : null;
}

export function StageBackdrop({ image, period }: { image: BackdropImage | null; period: string }) {
  const [last, setLast] = useState<BackdropImage | null>(null);
  const ready = image?.src ? image : null;
  useEffect(() => {
    if (ready) setLast(ready);
  }, [ready]);
  const shown = ready ?? last;
  const tint = tintOf(period, shown);
  return (
    <>
      {ready ? (
        <img key={ready.rev} className="vn-backdrop" src={ready.src!} alt="" />
      ) : last ? (
        <img className="vn-backdrop vn-backdrop-waiting" src={last.src!} alt="" />
      ) : (
        <div className="vn-backdrop vn-backdrop-black" />
      )}
      <div
        className="vn-tint"
        style={{ backgroundColor: tint?.color ?? "#fff", opacity: tint ? tint.opacity : 0 }}
        aria-hidden
      />
    </>
  );
}
