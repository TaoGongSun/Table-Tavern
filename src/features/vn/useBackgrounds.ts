// 舞台背景：按需讀背景庫的圖（data URL），聽背景佇列的狀態事件，生好或重生成完就重讀那張。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Backdrop, BackgroundStatus } from "../../shared/contracts/backend-contracts";

export const BACKGROUND_STATUS_EVENT = "vn-background-status";

export const backdropKey = (backdrop: Pick<Backdrop, "slug" | "period">) =>
  `${backdrop.slug}-${backdrop.period}`;

/** 一張背景：src＝null 是庫裡還沒有；rev 每讀到新圖加一（淡入以它為 key） */
export interface BackdropImage {
  src: string | null;
  rev: number;
}

export function useBackgrounds(worldId: string) {
  const [images, setImages] = useState<Record<string, BackdropImage>>({});
  const [status, setStatus] = useState<Record<string, BackgroundStatus>>({});
  const requested = useRef(new Set<string>());
  const rev = useRef(0);

  const read = useCallback(
    (slug: string, period: string) => {
      invoke<string | null>("vn_background_read", { worldId, slug, period })
        .then((src) =>
          setImages((current) => ({
            ...current,
            [backdropKey({ slug, period })]: { src, rev: src ? ++rev.current : 0 },
          })),
        )
        .catch(() => {});
    },
    [worldId],
  );

  useEffect(() => {
    let alive = true;
    requested.current = new Set();
    setImages({});
    setStatus({});
    invoke<BackgroundStatus[]>("vn_backgrounds_status", { worldId })
      .then((list) => alive && setStatus(Object.fromEntries(list.map((item) => [backdropKey(item), item]))))
      .catch(() => {});
    const off = listen<BackgroundStatus>(BACKGROUND_STATUS_EVENT, ({ payload }) => {
      if (!alive || payload.worldId !== worldId) return;
      setStatus((current) => ({ ...current, [backdropKey(payload)]: payload }));
      if (payload.state === "done") read(payload.slug, payload.period);
    });
    return () => {
      alive = false;
      void off.then((stop) => stop());
    };
  }, [worldId, read]);

  /** 第一次用到這個鍵才讀 */
  const load = useCallback(
    (backdrop: Backdrop) => {
      const key = backdropKey(backdrop);
      if (requested.current.has(key)) return;
      requested.current.add(key);
      read(backdrop.slug, backdrop.period);
    },
    [read],
  );

  const regenerate = useCallback(
    (backdrop: Backdrop) =>
      invoke("vn_background_regenerate", {
        worldId,
        location: backdrop.location,
        period: backdrop.period,
      }),
    [worldId],
  );

  return { images, status, load, regenerate };
}
