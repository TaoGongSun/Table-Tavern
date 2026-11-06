// 導覽頁截圖：`tour/<鍵>.<語系>.webp`、房間面板借用的說明書圖 `guide/shots/<鍵>.<語系>.webp`。
// 每個語系先找自己的圖，沒有再退英文圖（不拿另一種中文頂替）。沒有就回 null，頁面不畫那張、也不放假圖。
import type { Lang } from "../../i18n";

export type TourImages = Record<string, string>;

// 舊圖還在資料夾裡，但頁面不再用：不打進這個 map，就不會被打包出去。
export const TOUR_IMAGES: TourImages = import.meta.glob<string>(
  ["./tour/*.webp", "!./tour/{subscription,editor,roomHost,roomGuest}.*.webp"],
  { eager: true, query: "?url", import: "default" },
);
export const GUIDE_SHOTS: TourImages = import.meta.glob<string>("./guide/shots/*.webp", { eager: true, query: "?url", import: "default" });

/** 下載頁新拍的桌面圖是 1600×1136。借來的說明書桌面圖實檔是 1600×855、手機圖是 600×1300，各用自己的寬高留白。 */
export const SHOT_SIZE: Record<string, { width: number; height: number }> = {
  lobby: { width: 1600, height: 1136 },
  table: { width: 1600, height: 1136 },
  status: { width: 1600, height: 1136 },
  cardUi: { width: 1600, height: 1136 },
  oneLine: { width: 1600, height: 1136 },
  oneLineOutline: { width: 1600, height: 1136 },
  vnPlay: { width: 1600, height: 1136 },
  vnWizard: { width: 1600, height: 1136 },
  "room-04": { width: 1600, height: 855 },
  "room-15": { width: 600, height: 1300 },
};

export function shotSize(key: string): { width: number; height: number } {
  return SHOT_SIZE[key] ?? { width: 1600, height: 1136 };
}

export function pickLocalized(images: TourImages, folder: string, key: string, lang: Lang): string | null {
  return images[`${folder}${key}.${lang}.webp`] ?? (lang === "en" ? null : (images[`${folder}${key}.en.webp`] ?? null));
}

export function pickTourImage(images: TourImages, key: string, lang: Lang): string | null {
  return pickLocalized(images, "./tour/", key, lang);
}

export function pickGuideImage(images: TourImages, key: string, lang: Lang): string | null {
  return pickLocalized(images, "./guide/shots/", key, lang);
}
