import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Known = { worldId: string; gmOnly: boolean; nominates: boolean } | null;
/** 後端 `gm_table_mode` 的回傳 */
type GmTableMode = { gm_only: boolean; nominates: boolean };

function isMode(value: unknown): value is GmTableMode {
  const mode = value as Partial<GmTableMode> | null;
  return typeof mode === "object" && mode !== null && typeof mode.gm_only === "boolean" && typeof mode.nominates === "boolean";
}

/**
 * 這桌是否只走 GM 一條敘事線（介面接管桌，與後端 `data::is_interface_takeover` 同一判定）〔作者裁決 2026-10-27〕。
 * 判定綁著讀回時的桌：換桌後、新桌讀回前是 undefined（未就緒），上一桌的值不會帶過來。
 * 同一次讀回帶 nominates：GM 回合會點名下一位（一般旁白格式）。讀取失敗保留這桌上次的結果；
 * 從沒讀成就當一般桌（角色回合在後端仍會被拒收）、不點名（「送出並接話」不給）。
 */
/** 判定看得到的卡片：主區名單＋全部卡的封存／auto_hidden 旗標 */
interface CastFlags {
  active: { id: string }[];
  list: { id: string; archived: boolean; auto_hidden: boolean }[];
}

/** 名單鍵：主區 id、或任一張卡的封存／auto_hidden 旗標變了就不同 */
export function castKey(cast: CastFlags): string {
  return [
    cast.active.map((card) => card.id).join(","),
    cast.list.map((card) => `${card.id}:${card.archived ? 1 : 0}${card.auto_hidden ? 1 : 0}`).join(","),
  ].join("|");
}

export function useGmOnlyTable(worldId: string, cast?: CastFlags) {
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
    let mode: GmTableMode | null = null;
    try {
      const read = await invoke<unknown>("gm_table_mode", { worldId: target });
      if (isMode(read)) mode = read;
    } catch {
      // 下面統一處理
    }
    if (mine !== load.current) return;
    if (mode === null) {
      // 讀不到：同一桌保留上次讀成的結果；這桌從沒讀成就當一般桌、不點名（藏「送出並接話」，不多花錢）
      if (knownRef.current?.worldId === target) return;
      mode = { gm_only: false, nominates: false };
    }
    knownRef.current = { worldId: target, gmOnly: mode.gm_only, nominates: mode.nominates };
    setKnownState(knownRef.current);
  }, []);

  useEffect(() => {
    if (worldId) void refresh(worldId);
  }, [worldId, refresh]);

  // 「會不會點名」看卡片格式腳本，後端略過封存與 auto_hidden 旗標的卡（旗標換幕才變，劇情帶出場不改它）：
  // 名單鍵一變就重讀這桌；第一次讀取由上面換桌那支負責
  const key = cast ? castKey(cast) : "";
  const seenKey = useRef<string | null>(null);
  useEffect(() => {
    const previous = seenKey.current;
    seenKey.current = key;
    if (previous !== null && previous !== key && current.current) void refresh(current.current);
  }, [key, refresh]);

  /** 回合入口的閘：這桌的判定還沒讀回就不做（換桌瞬間空對象的送出會留下沒人回的半回合） */
  const whenReady = useCallback(
    <A extends unknown[]>(fn: (...args: A) => Promise<void>) =>
      (...args: A) =>
        knownRef.current?.worldId === current.current && current.current !== "" ? fn(...args) : Promise.resolve(),
    [],
  );

  const here = known !== null && known.worldId === worldId ? known : null;
  return {
    gmOnly: here?.gmOnly,
    /** GM 回合會點名（判定未就緒時 false） */
    nominates: here?.nominates === true,
    refresh,
    whenReady,
  };
}
