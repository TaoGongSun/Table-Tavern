// 舞台背景層：目前背景的圖好了就淡入；還沒好＝前一張模糊頂著，第一張都還沒有＝黑幕（字卡由 Stage 疊在上面）。
import { useEffect, useState } from "react";
import type { BackdropImage } from "./useBackgrounds";

export function StageBackdrop({ image }: { image: BackdropImage | null }) {
  const [last, setLast] = useState<BackdropImage | null>(null);
  const ready = image?.src ? image : null;
  useEffect(() => {
    if (ready) setLast(ready);
  }, [ready]);
  if (ready) return <img key={ready.rev} className="vn-backdrop" src={ready.src!} alt="" />;
  if (last) return <img className="vn-backdrop vn-backdrop-waiting" src={last.src!} alt="" />;
  return <div className="vn-backdrop vn-backdrop-black" />;
}
