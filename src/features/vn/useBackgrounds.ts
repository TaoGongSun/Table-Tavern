// 舞台背景：按需讀背景庫的圖（該時段專屬圖，沒有就同地點基底），聽背景佇列的狀態事件；
// 一個地點的圖生好、重生成完或上傳替換，重讀這個地點所有用過的時段（基底被多個時段共用）。舞台與美術一覽共用一份。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import type { ArtStatus, Backdrop, BackgroundStatus, BackgroundView } from "../../shared/contracts/backend-contracts";

export const BACKGROUND_STATUS_EVENT = "vn-background-status";

export const backdropKey = (backdrop: Pick<Backdrop, "slug" | "period">) =>
  `${backdrop.slug}-${backdrop.period}`;

/** 一張背景：src＝null 是庫裡還沒有；period＝圖實際的時段（與要的不同＝基底）；rev 每讀到新圖加一（淡入以它為 key） */
export interface BackdropImage {
  src: string | null;
  period: string | null;
  rev: number;
}

export function useBackgrounds(worldId: string) {
  const [pause, setPause] = useState<ArtStatus["pause"]>(null);
  const [images, setImages] = useState<Record<string, BackdropImage>>({});
  const [status, setStatus] = useState<Record<string, BackgroundStatus>>({});
  /** 讀過的鍵：slug → 時段 */
  const requested = useRef(new Map<string, Set<string>>());
  const rev = useRef(0);

  const read = useCallback(
    (slug: string, period: string) => {
      invoke<BackgroundView | null>("vn_background_read", { worldId, slug, period })
        .then((view) =>
          setImages((current) => {
            const key = backdropKey({ slug, period });
            // 同一張圖（同地點基底換時段）不換 rev，舞台不重新淡入
            const same = view && current[key]?.src === view.src;
            return {
              ...current,
              [key]: { src: view?.src ?? null, period: view?.period ?? null, rev: same ? current[key].rev : view ? ++rev.current : 0 },
            };
          }),
        )
        .catch(() => {});
    },
    [worldId],
  );

  useEffect(() => {
    let alive = true;
    requested.current = new Map();
    setImages({});
    setStatus({});
    setPause(null);
    const readPause = () => invoke<ArtStatus>("vn_art_status", { worldId }).then((art) => { if (alive) setPause(art?.pause ?? null); }).catch(() => {});
    void readPause();
    const flow = listen("vn-art-flow-status", () => void readPause());
    invoke<BackgroundStatus[]>("vn_backgrounds_status", { worldId })
      .then((list) => alive && setStatus(Object.fromEntries(list.map((item) => [backdropKey(item), item]))))
      .catch(() => {});
    const off = listen<BackgroundStatus>(BACKGROUND_STATUS_EVENT, ({ payload }) => {
      if (!alive || payload.worldId !== worldId) return;
      setStatus((current) => ({ ...current, [backdropKey(payload)]: payload }));
      if (payload.state !== "done") return;
      for (const period of requested.current.get(payload.slug) ?? []) read(payload.slug, period);
    });
    return () => {
      alive = false;
      void flow.then((stop) => stop());
      void off.then((stop) => stop());
    };
  }, [worldId, read]);

  /** 第一次用到這個鍵才讀 */
  const load = useCallback(
    (backdrop: Backdrop) => {
      const periods = requested.current.get(backdrop.slug) ?? new Set<string>();
      if (periods.has(backdrop.period)) return;
      requested.current.set(backdrop.slug, periods.add(backdrop.period));
      read(backdrop.slug, backdrop.period);
    },
    [read],
  );

  /** 這個鍵的佇列狀態；自己沒排過就看同地點的（基底按地點排） */
  const statusOf = useCallback(
    (backdrop: Pick<Backdrop, "slug" | "period">) =>
      status[backdropKey(backdrop)] ?? Object.values(status).find((item) => item.slug === backdrop.slug),
    [status],
  );

  /** status＝各鍵（slug－時段）的佇列狀態，美術一覽逐格看 */
  const retry = useCallback((backdrop: Backdrop) => pause
    ? invoke("vn_art_retry", { worldId })
    : invoke("vn_background_regenerate", { worldId, slug: backdrop.slug, period: backdrop.period }), [worldId, pause]);
  return { images, status, statusOf, load, pause, retry };
}
