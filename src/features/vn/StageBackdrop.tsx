// 舞台背景層：目前背景的圖好了就淡入；還沒好＝前一張模糊頂著，第一張都還沒有＝漸層與左上地點膠囊。
// 圖是同地點基底（實際時段與要的不同）就疊該時段的色調遮罩（plans/vn-setup-wizard.md §3）：蓋背景（立繪只在夜晚壓半強度，見 Stage），
// 換時段時遮罩漸變；卡自訂時段沒有對應色就不套。
// 天氣（plans/vn-weather-mask.md §3）：去飽和＋壓暗調色層在時段遮罩之下（黃昏的橘色留著），霧層也在遮罩之下（夜霧不發亮）；
// 雨層由 Stage 疊在立繪之上。調色不看圖是基底還是專屬圖，一律套。
import { t } from "../../i18n";
import { useEffect, useState } from "react";
import type { BackdropImage } from "./useBackgrounds";

/** 時段色調：multiply 疊色（白＝不變），opacity 控制強度 */
const TINTS: Record<string, { color: string; opacity: number }> = {
  morning: { color: "#ccd7f0", opacity: 0.75 },
  dusk: { color: "#ff9a5c", opacity: 0.75 },
  night: { color: "#22305f", opacity: 0.88 },
};

export function tintOf(period: string, image: BackdropImage | null) {
  if (!image?.period || image.period === period) return null;
  return TINTS[period] ?? null;
}

/** 天氣調色：saturation 疊灰去飽和、multiply 壓暗（強度＝opacity）；陰雨的壓暗上暗下亮，天空沉下去 */
const WEATHER: Record<string, { desat: number; dim: string; top?: string; dimOpacity: number; tintCap?: number }> = {
  overcast: { desat: 0.8, dim: "#b4b8be", top: "#8d939b", dimOpacity: 0.45, tintCap: 0.6 },
  fog: { desat: 0.6, dim: "#c4c9cf", dimOpacity: 0.3, tintCap: 0.55 },
  rain: { desat: 0.65, dim: "#7c8a9c", top: "#5f6c7d", dimOpacity: 0.6, tintCap: 0.6 },
};

/**
 * 天氣與時段一起時：有時段遮罩就去飽和減半、清晨黃昏的遮罩壓淡（陰雨看不到晚霞），
 * 免得「近灰階＋單色染色」成了懷舊色調、被讀成回憶場景。夜晚遮罩不動（夜色本來就近單色）。
 */
export function weatherGrade(weather: string, tint: { color: string; opacity: number } | null, period: string) {
  const sky = WEATHER[weather];
  if (!sky) return { sky: null, desat: 0, tint };
  const capped = tint && sky.tintCap !== undefined && period !== "night" ? Math.min(tint.opacity, sky.tintCap) : tint?.opacity;
  return {
    sky,
    desat: tint ? sky.desat / 2 : sky.desat,
    tint: tint && { color: tint.color, opacity: capped! },
  };
}

/** 霧氣貼圖：stitchTiles 讓 960px 一格左右接得上，霧層平移剛好一格不跳 */
const FOG = `url("data:image/svg+xml,${encodeURIComponent(
  "<svg xmlns='http://www.w3.org/2000/svg' width='960' height='540'><filter id='f'>" +
    "<feTurbulence type='fractalNoise' baseFrequency='0.004 0.009' numOctaves='3' stitchTiles='stitch'/>" +
    "<feColorMatrix values='0 0 0 0 0.9 0 0 0 0 0.92 0 0 0 0 0.94 0 0 0 1.7 -0.5'/></filter>" +
    "<rect width='100%' height='100%' filter='url(#f)'/></svg>",
)}")`;

/** 立繪的色調（只疊一層）：夜晚＝背景遮罩的半強度；非夜晚只有下雨蒙一點灰（陰、霧不壓人物） */
export function spriteTint(period: string | undefined, weather: string): { color: string; opacity: number } | null {
  if (period === "night") return { color: TINTS.night.color, opacity: TINTS.night.opacity / 2 };
  return weather === "rain" ? { color: "#4a525c", opacity: 0.18 } : null;
}

/** 雨絲遮罩貼圖：size 見方的一格內固定位置的短線（決定性亂數），雨層平移剛好一格不跳 */
function rainTile(size: number, count: number, length: number, width: number) {
  let seed = size;
  const next = () => (seed = (seed * 9301 + 49297) % 233280) / 233280;
  // 跨格底邊的線在上一格（y − size）再畫一次，接縫不斷
  const lines = Array.from({ length: count }, () => [Math.round(next() * size), Math.round(next() * size)])
    .flatMap(([x, y]) => [y, y - size].map((top) => `<line x1='${x}' y1='${top}' x2='${x}' y2='${top + length}'/>`))
    .join("");
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${size}' height='${size}'><g stroke='#000' stroke-width='${width}' stroke-linecap='round'>${lines}</g></svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}
const RAIN_FAR = rainTile(160, 9, 14, 1);
const RAIN_NEAR = rainTile(260, 5, 28, 1.5);

/** 雨層（Stage 疊在立繪之上、對話框之下）：遠近兩片，各以自己的格高平移；夜雨偏冷灰藍且較淡 */
export function RainLayer({ on, period }: { on: boolean; period: string | undefined }) {
  return (
    <div className={`vn-rain${on ? " vn-rain-on" : ""}${period === "night" ? " vn-rain-night" : ""}`} aria-hidden>
      <div className="vn-rain-sheet vn-rain-far" style={{ maskImage: RAIN_FAR, WebkitMaskImage: RAIN_FAR }} />
      <div className="vn-rain-sheet vn-rain-near" style={{ maskImage: RAIN_NEAR, WebkitMaskImage: RAIN_NEAR }} />
    </div>
  );
}

export function StageBackdrop({ image, period, weather, location, state, onRetry }: {
  image: BackdropImage | null; period: string; weather: string; location?: string; state?: string; onRetry?: () => void;
}) {
  const [last, setLast] = useState<BackdropImage | null>(null);
  const ready = image?.src ? image : null;
  useEffect(() => {
    if (ready) setLast(ready);
  }, [ready]);
  const shown = ready ?? last;
  const fallback = ({ morning: "#293449", day: "#293b46", dusk: "#48312f", night: "#18213b" } as Record<string, string>)[period] ?? "#293b46";
  const { sky, desat, tint } = weatherGrade(weather, tintOf(period, shown), period);
  return (
    <>
      {ready ? (
        <img key={ready.rev} className="vn-backdrop" src={ready.src!} alt="" />
      ) : last ? (
        <img className="vn-backdrop vn-backdrop-waiting" src={last.src!} alt="" />
      ) : (
        <div className="vn-backdrop vn-backdrop-placeholder" style={{ background: `linear-gradient(160deg, ${fallback}, #10131c 85%)` }} />
      )}
      {!shown && location && <div key={location} className="vn-location">{location}</div>}
      {(state === "failed" || state === "paused") && onRetry && <button type="button" className="btn btn-sm vn-background-retry" onClick={(event) => { event.stopPropagation(); onRetry(); }}>
        {t(state === "paused" ? "vnBackgroundQuota" : "vnBackgroundRetry")}
      </button>}
      <div className="vn-weather-desat" style={{ opacity: desat }} aria-hidden />
      <div
        className="vn-weather-dim"
        style={{
          backgroundColor: sky?.dim ?? "#fff",
          backgroundImage: sky?.top ? `linear-gradient(to bottom, ${sky.top}, ${sky.dim} 60%)` : "none",
          opacity: sky?.dimOpacity ?? 0,
        }}
        aria-hidden
      />
      <div className={`vn-fog${weather === "fog" ? " vn-fog-on" : ""}`} style={{ backgroundImage: FOG }} aria-hidden />
      <div
        className="vn-tint"
        style={{ backgroundColor: tint?.color ?? "#fff", opacity: tint ? tint.opacity : 0 }}
        aria-hidden
      />
    </>
  );
}
