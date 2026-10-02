// 桌卡封面的懶載：卡片第一次靠近視窗（約 200px）才讀圖，之後不再讀。
// 不跨次快取——Lobby 每次掛載都是全新的一批卡，進出大廳就重讀一次，改了陣容或換了圖永遠是最新的。
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { CharacterMeta } from "../characters/card-model";
import {
  coverCandidates,
  imageDecodes,
  loadCoverImages,
  type CoverCandidate,
} from "./table-cover";

const VISIBLE_MARGIN = "200px";

async function readCandidate(worldId: string, candidate: CoverCandidate): Promise<string | null> {
  const base64 =
    candidate.kind === "gm"
      ? await invoke<string | null>("read_gm_image", { worldId })
      : await invoke<string | null>("read_character_image", {
          worldId,
          characterId: candidate.id,
        });
  return base64 ? `data:image/png;base64,${base64}` : null;
}

async function loadTableCover(worldId: string, isStale: () => boolean) {
  // 需修復的桌讀不了名單：當成沒有角色，只剩 GM 圖或書皮
  const [characters, appearances] = await Promise.all([
    invoke<CharacterMeta[]>("list_characters", { worldId }).catch(() => []),
    invoke<{ character_ids: string[] }>("scene_appearances", { worldId }).catch(() => ({
      character_ids: [] as string[],
    })),
  ]);
  return loadCoverImages(
    coverCandidates(characters, new Set(appearances.character_ids)),
    (candidate) => readCandidate(worldId, candidate),
    imageDecodes,
    isStale,
  );
}

/**
 * 回傳掛在封面元素上的 ref 與目前的圖：null＝還沒載入（畫皮革底），
 * 陣列＝載入完成（空陣列＝一張都沒有，退書皮）。
 */
export function useTableCover(worldId: string) {
  const ref = useRef<HTMLButtonElement>(null);
  const [near, setNear] = useState(false);
  const [images, setImages] = useState<string[] | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    if (typeof IntersectionObserver === "undefined") {
      setNear(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;
        setNear(true);
        observer.disconnect();
      },
      { rootMargin: VISIBLE_MARGIN },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!near) return;
    // 桌 id 換了或元件卸載：這一輪還在飛的回應一律丟掉
    let stale = false;
    setImages(null);
    void loadTableCover(worldId, () => stale).then((loaded) => {
      if (!stale && loaded) setImages(loaded);
    });
    return () => {
      stale = true;
    };
  }, [near, worldId]);

  return { ref, images };
}
