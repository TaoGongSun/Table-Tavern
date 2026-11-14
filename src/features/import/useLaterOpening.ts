// 空桌的「選擇開場白」入口露不露：這一幕逐字稿空著時問後端（還沒開始玩、留有帶開場白的匯入原檔）。
// 收據換過（匯入、復原匯入）就重問；按下去時後端會再查一次，這裡只決定按鈕出不出現。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { currentLang } from "../../i18n";
import type { ImportReceiptSummary } from "./useImportController";

export function useLaterOpening(
  worldId: string,
  storyEmpty: boolean,
  receipts: ImportReceiptSummary[],
): boolean {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    setReady(false);
    if (!storyEmpty || worldId === "") return;
    let live = true;
    invoke<unknown>("table_openings", { worldId, lang: currentLang() })
      .then((found) => live && setReady(found !== null))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [worldId, storyEmpty, receipts]);
  return ready;
}
