import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Known = { worldId: string; gmOnly: boolean } | null;

/**
 * 這桌是否只走 GM 一條敘事線（介面接管桌，與後端 `data::is_interface_takeover` 同一判定）〔作者裁決 2026-10-27〕。
 * 判定綁著讀回時的桌：換桌後、新桌讀回前是 undefined（未就緒），上一桌的值不會帶過來。
 * 讀取失敗當一般桌：角色回合在後端仍會被拒收。
 */
export function useGmOnlyTable(worldId: string) {
  const [known, setKnownState] = useState<Known>(null);
  // 呼叫當下讀得到的最新值（回合入口的閉包可能是上一次 render 的）
  const knownRef = useRef<Known>(null);
  const current = useRef(worldId);
  current.current = worldId;
  // 讀取世代：換桌或又讀一次後，晚到的舊回應不寫進來
  const load = useRef(0);

  /** 重讀這桌（重構套用、撤銷後）；已換走的桌不讀，免得蓋掉現在這桌正在進行的讀取 */
  const refresh = useCallback(async (target: string) => {
    if (target !== current.current) return;
    const mine = ++load.current;
    let gmOnly = false;
    try {
      gmOnly = await invoke<boolean>("interface_takeover_table", { worldId: target });
    } catch {
      // 當一般桌
    }
    if (mine !== load.current) return;
    knownRef.current = { worldId: target, gmOnly };
    setKnownState(knownRef.current);
  }, []);

  useEffect(() => {
    if (worldId) void refresh(worldId);
  }, [worldId, refresh]);

  /** 回合入口的閘：這桌的判定還沒讀回就不做（換桌瞬間空對象的送出會留下沒人回的半回合） */
  const whenReady = useCallback(
    <A extends unknown[]>(fn: (...args: A) => Promise<void>) =>
      (...args: A) =>
        knownRef.current?.worldId === current.current && current.current !== "" ? fn(...args) : Promise.resolve(),
    [],
  );

  return {
    gmOnly: known !== null && known.worldId === worldId ? known.gmOnly : undefined,
    refresh,
    whenReady,
  };
}
