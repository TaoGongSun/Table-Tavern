import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SceneBudgetReply } from "./scene-budget";

/** 設定內容的短指紋（FNV-1a 32）：請求帶著它，回應原樣帶回，用來核對「這是目前設定下算的」 */
export function configTag(configKey: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < configKey.length; index += 1) {
    hash ^= configKey.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}

// 換幕容量的取得：進桌、每輪結束（不忙且則數變了）、換幕／退回／分岔（幕變了）、改設定時重抓。
// 每次切換（換桌、換幕、改設定、忙／不忙、則數變）都先撤銷在途請求：清理時把序號推進、
// 並把閉包標成失效，晚回的一律丟掉。回應另核對桌、幕、序號，以及帶回的設定指紋必須等於「目前設定」的——
// 後端只在量測所用的設定快照就是前端這份時才帶回指紋，所以收得到的一定是在目前設定下量的。
// 換桌、換幕、改設定先清掉舊值——刷新期間不沿用舊容量（送出時反正以後端關卡為準）。
export function useSceneBudget(
  worldId: string,
  scene: number,
  eventCount: number,
  busy: boolean,
  config: unknown,
): SceneBudgetReply | null {
  // 設定整份當世代鍵：換傳輸／模型／檔位都會重量
  const configKey = JSON.stringify(config);
  const [budget, setBudget] = useState<SceneBudgetReply | null>(null);
  const seq = useRef(0);
  const expectedTag = useRef(configTag(configKey));

  useEffect(() => {
    expectedTag.current = configTag(configKey);
    setBudget(null);
  }, [configKey]);

  useEffect(() => {
    setBudget(null);
  }, [worldId, scene]);

  useEffect(() => {
    const requestSeq = ++seq.current;
    const tag = configTag(configKey);
    let live = true;
    if (!busy && worldId) {
      // 設定本身也送去：後端用同一份快照量測，快照不是這份（樂觀更新還沒寫成）就不帶回指紋
      invoke<SceneBudgetReply>("scene_budget", { worldId, requestSeq, configTag: tag, config })
        .then((reply) => {
          if (!live || reply.requestSeq !== seq.current) return;
          if (reply.worldId !== worldId || reply.scene !== scene) return;
          if (reply.configTag !== expectedTag.current) return;
          setBudget(reply);
        })
        .catch(() => {
          /* 量不到就不提醒；送出仍由後端關卡把關 */
        });
    }
    return () => {
      live = false;
      seq.current += 1;
    };
    // config 物件身分不算，只看內容（configKey）
  }, [worldId, scene, eventCount, busy, configKey]);

  return budget;
}
