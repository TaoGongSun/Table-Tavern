// 精靈 ④ 開演：一進來就排首幕背景，生成中顯示進度、好了顯示預覽（plans/vn-mode-pkg3.md §2）。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import type { Backdrop } from "../../shared/contracts/backend-contracts";
import { backdropKey, useBackgrounds } from "./useBackgrounds";
import "./vn-sprites.css";

export function VnStartStep({ worldId }: { worldId: string }) {
  const [first, setFirst] = useState<Backdrop | null>(null);
  const { images, status, load } = useBackgrounds(worldId);
  useEffect(() => {
    invoke<Backdrop[]>("vn_backgrounds_fill", { worldId, scene: null })
      .then((used) => setFirst(used[0] ?? null))
      .catch(() => {});
  }, [worldId]);
  useEffect(() => {
    if (first) load(first);
  }, [first, load]);
  const key = first && backdropKey(first);
  const src = key ? images[key]?.src : null;
  return (
    <div className="vn-start-step">
      <p>{t("vnStepStartBody")}</p>
      {first &&
        (src ? (
          <img className="vn-start-preview" src={src} alt={first.location} />
        ) : (
          <p className="vn-start-progress">
            {t(status[key!]?.state === "failed" ? "vnBackgroundFailed" : "vnBackgroundPreparing")}
          </p>
        ))}
    </div>
  );
}
