// 桌卡封面的選圖與排版。純函式，不碰 invoke：載入層（useTableCover）把讀圖動作注入進來，
// 這裡只管「要試哪些圖、依什麼順序、湊到幾張為止、怎麼排」。
import { isCharacterHidden } from "../characters/character-visibility";

/** 封面最多拼幾格 */
export const COVER_MAX = 2;

export type CoverCandidate = { kind: "gm" } | { kind: "character"; id: string };

interface CoverCharacter {
  id: string;
  archived: boolean;
  auto_hidden: boolean;
  show_image: boolean;
}

/**
 * 候選順序：GM 圖第一（有沒有圖要實際讀了才知道，所以一律排進來），接著依陣容順序、
 * 目前可見（沿用陣容欄的隱藏判準）且開著 show_image 的角色。不在這裡截斷：
 * 前面的候選可能讀不到或解碼失敗，後面的要能遞補。
 */
export function coverCandidates(
  characters: readonly CoverCharacter[],
  appearances: ReadonlySet<string>,
): CoverCandidate[] {
  return [
    { kind: "gm" },
    ...characters
      .filter((character) => !isCharacterHidden(character, appearances) && character.show_image)
      .map((character): CoverCandidate => ({ kind: "character", id: character.id })),
  ];
}

export type CoverLayout = "book" | "fill" | "tiles";

/** 0 張＝GM 書皮；1 張鋪滿；2 張左右並排各半寬 */
export function coverLayout(imageCount: number): CoverLayout {
  if (imageCount <= 0) return "book";
  return imageCount === 1 ? "fill" : "tiles";
}

/**
 * 依候選順序取圖，湊滿 COVER_MAX 張就停。每一輪只同時讀「還缺幾張」那麼多個：
 * 全成功時不多讀，有失敗時下一輪再往後補，最終順序仍等同候選順序。
 * 「成功」＝讀得到且能解碼。isStale 回 true（桌換了、元件卸載）就整批丟掉，回 null。
 */
export async function loadCoverImages(
  candidates: readonly CoverCandidate[],
  fetchImage: (candidate: CoverCandidate) => Promise<string | null>,
  decodes: (src: string) => Promise<boolean>,
  isStale: () => boolean,
): Promise<string[] | null> {
  const images: string[] = [];
  let next = 0;
  while (images.length < COVER_MAX && next < candidates.length) {
    const batch = candidates.slice(next, next + (COVER_MAX - images.length));
    next += batch.length;
    const results = await Promise.all(
      batch.map(async (candidate) => {
        try {
          const src = await fetchImage(candidate);
          return src && (await decodes(src)) ? src : null;
        } catch {
          return null;
        }
      }),
    );
    if (isStale()) return null;
    for (const src of results) if (src) images.push(src);
  }
  return images;
}

/** 以 <img> 實際解碼為準：壞圖（讀得到 base64 但不是圖）在這裡就被擋下，不會上畫面 */
export async function imageDecodes(src: string): Promise<boolean> {
  const image = new Image();
  image.src = src;
  try {
    await image.decode();
    return true;
  } catch {
    return false;
  }
}
