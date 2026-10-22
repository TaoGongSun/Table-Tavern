// 導覽頁截圖：`tour/<鍵>.<zh-TW|en>.webp` 經 Vite 打包進 /assets（計畫 desktop-tour 第四節）。
// 只拍了繁中與英文兩套：繁中、簡中頁面用繁中圖，其餘語系用英文圖；對應的圖不在就回 null（頁面退圖示卡）。
import type { Lang } from "../../i18n";

export type TourImages = Record<string, string>;

export const TOUR_IMAGES: TourImages = import.meta.glob<string>("./tour/*.webp", { eager: true, query: "?url", import: "default" });

export function pickTourImage(images: TourImages, key: string, lang: Lang): string | null {
  const suffix = lang === "zh-TW" || lang === "zh-CN" ? "zh-TW" : "en";
  return images[`./tour/${key}.${suffix}.webp`] ?? null;
}
