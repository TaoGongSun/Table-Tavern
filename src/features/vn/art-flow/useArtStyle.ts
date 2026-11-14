import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PlayerStyle } from "../../../shared/contracts/backend-contracts";
import { registerArtStyleSaver } from "./art-style-save";

const same = (a: PlayerStyle, b: PlayerStyle) => a.style === b.style && a.avoid === b.avoid;

/**
 * 這桌畫風的草稿與存檔：`stored`＝狀態讀回來的值（第一次到才當起點）。存檔排成一條鏈，
 * 離開輸入框與生圖前的強制存檔不會交錯；跟上次送出的一樣就不再送。
 */
export function useArtStyle(worldId: string, stored: PlayerStyle | null) {
  const [value, setValue] = useState<PlayerStyle | null>(null);
  const [state, setState] = useState<"idle" | "saved" | "failed">("idle");
  const [changed, setChanged] = useState(false);
  const valueRef = useRef(value);
  valueRef.current = value;
  const sent = useRef<PlayerStyle | null>(null);
  const initial = useRef<PlayerStyle | null>(null);
  const chain = useRef<Promise<boolean>>(Promise.resolve(true));

  useEffect(() => {
    if (stored && initial.current === null) {
      initial.current = stored;
      sent.current = stored;
      setValue(stored);
    }
  }, [stored]);

  const save = useCallback(() => {
    chain.current = chain.current.then(async () => {
      const next = valueRef.current;
      if (!next || (sent.current && same(sent.current, next))) return true;
      try {
        const saved = await invoke<PlayerStyle>("vn_art_style_set", { worldId, style: next });
        sent.current = next;
        setState("saved");
        setChanged(initial.current !== null && !same(initial.current, saved));
        // 後端清洗過（去頭尾空白、截長）：輸入框還是送出時的內容才換成實際存下的
        setValue((current) => (current && same(current, next) ? saved : current));
        if (same(next, saved)) sent.current = saved;
        return true;
      } catch {
        setState("failed");
        return false;
      }
    });
    return chain.current;
  }, [worldId]);

  useEffect(() => registerArtStyleSaver(worldId, save), [worldId, save]);

  const edit = useCallback((next: PlayerStyle) => {
    setValue(next);
    setState("idle");
  }, []);
  return { value, edit, save, state, changed };
}
