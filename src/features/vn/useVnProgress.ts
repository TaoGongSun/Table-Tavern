// VN 個人進度（vn-mode 包 6 §2.6、§2.7）：進桌讀一次 `vn-progress.json`，已讀與閱讀位置在前端累積、
// 1.5 秒防抖整批寫回（離開播放器時再沖一次）；寫失敗不打擾玩家（只影響 skip 與續讀）。存檔增刪讀走後端。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type { VnProgress, VnSave } from "../../shared/contracts/backend-contracts";

const FLUSH_MS = 1500;

export function useVnProgress(worldId: string) {
  const [loaded, setLoaded] = useState(false);
  const [saves, setSaves] = useState<VnSave[]>([]);
  const [positions, setPositions] = useState<Record<string, string>>({});
  const read = useRef(new Set<string>());
  const [readVersion, setReadVersion] = useState(0);
  const pending = useRef<{ keys: string[]; position: { scene: number; key: string } | null }>({
    keys: [],
    position: null,
  });
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const flush = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const { keys, position } = pending.current;
    if (keys.length === 0 && !position) return;
    pending.current = { keys: [], position: null };
    invoke("vn_progress_write", {
      worldId,
      readAdd: keys,
      scene: position?.scene ?? null,
      position: position?.key ?? null,
    }).catch(() => {});
  }, [worldId]);

  useEffect(() => {
    let live = true;
    setLoaded(false);
    read.current = new Set();
    invoke<VnProgress>("vn_progress_read", { worldId })
      .then((progress) => {
        if (!live) return;
        read.current = new Set(progress?.read ?? []);
        setSaves(progress?.saves ?? []);
        setPositions(progress?.positions ?? {});
      })
      .catch(() => {})
      .finally(() => live && setLoaded(true));
    return () => {
      live = false;
      flush();
    };
  }, [worldId, flush]);

  const schedule = useCallback(() => {
    if (!timer.current) timer.current = setTimeout(flush, FLUSH_MS);
  }, [flush]);

  /** 這格讀過了（同一鍵只記一次） */
  const markRead = useCallback((key: string) => {
    if (read.current.has(key)) return;
    read.current.add(key);
    pending.current.keys.push(key);
    setReadVersion((n) => n + 1);
    schedule();
  }, [schedule]);

  /** 目前這幕讀到哪一格（正式格鍵） */
  const setPosition = useCallback((scene: number, key: string) => {
    pending.current.position = { scene, key };
    setPositions((now) => (now[String(scene)] === key ? now : { ...now, [String(scene)]: key }));
    schedule();
  }, [schedule]);

  const isRead = useCallback((key: string) => read.current.has(key), []);

  const reloadSaves = useCallback(async () => {
    const progress = await invoke<VnProgress>("vn_progress_read", { worldId });
    setSaves(progress?.saves ?? []);
  }, [worldId]);

  return { loaded, saves, positions, isRead, readVersion, markRead, setPosition, reloadSaves };
}
