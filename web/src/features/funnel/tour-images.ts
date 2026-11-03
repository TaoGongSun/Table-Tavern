// 導覽頁截圖：`tour/<鍵>.<zh-TW|zh-CN|en>.webp` 經 Vite 打包進 /assets（計畫 desktop-tour 第四節）。
// 繁中頁用繁中圖、簡中頁只用簡中圖（簡中頁以多人連線為主軸，另拍一套；沒拍的特色退圖示卡，不借繁中圖），其餘語系用英文圖；
// 對應的圖不在就回 null（頁面退圖示卡）。
import type { Lang } from "../../i18n";

export type TourImages = Record<string, string>;

export const TOUR_IMAGES: TourImages = import.meta.glob<string>("./tour/*.webp", { eager: true, query: "?url", import: "default" });

export function pickTourImage(images: TourImages, key: string, lang: Lang): string | null {
  const suffix = lang === "zh-TW" || lang === "zh-CN" ? lang : "en";
  return images[`./tour/${key}.${suffix}.webp`] ?? null;
}
