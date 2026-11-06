// 多人房間玩法說明（`#guide/room`）：房主、朋友、常見問題三段，頂部錨點跳段，每小節配一張截圖。
// 內容是 `guide/room.<語系>.md`，截圖是 `guide/shots/room-XX.<語系>.webp`；繁中、簡中的文章用自己的，其餘語系用英文。
// 截圖每個語系先找自己的，沒有再退英文圖。頁首「回到下載頁」連回 `#download/room`。
// 截圖編號 12 起是手機畫面，排得窄一點；對應的圖不在就不畫那張（03 露本機網址，重拍前先不放）。
import { useMemo, useState } from "react";
import { getLang, t } from "../../i18n";
import { downloadHash } from "./download-route";
import { inline, parseGuide } from "./guide-markdown";
import { pickGuideImage } from "./tour-images";
import { useOverlayPage } from "./useOverlayPage";

const GUIDES = import.meta.glob<string>("./guide/room.*.md", { eager: true, query: "?raw", import: "default" });
const SHOTS = import.meta.glob<string>("./guide/shots/*.webp", { eager: true, query: "?url", import: "default" });

const PHONE_FROM = 12;

export function RoomGuide({ onClose, shots = SHOTS }: { onClose: () => void; shots?: Record<string, string> }) {
  const closeButton = useOverlayPage(onClose);
  const lang = getLang();
  const suffix = lang === "zh-TW" || lang === "zh-CN" ? lang : "en";
  const blocks = useMemo(() => parseGuide(GUIDES[`./guide/room.${suffix}.md`] ?? GUIDES["./guide/room.en.md"] ?? ""), [suffix]);
  const [failed, setFailed] = useState<ReadonlySet<string>>(() => new Set());
  const title = blocks.find((block) => block.kind === "title")?.text ?? "";
  const sections = blocks.flatMap((block) => (block.kind === "section" ? [block] : []));

  return (
    <div className="overlay tour-overlay" role="dialog" aria-modal="true" aria-labelledby="guide-title" data-testid="room-guide">
      <header className="tour-header">
        <div className="tour-inner tour-header-row">
          <span className="tour-brand">Table Tavern</span>
          <span className="tour-header-actions">
            <a href={downloadHash("room")} data-testid="guide-back">
              {t("guideBackDownload")}
            </a>
            <button type="button" className="ghost" ref={closeButton} onClick={onClose}>
              {t("downloadClose")}
            </button>
          </span>
        </div>
      </header>
      <article className="tour-inner guide">
        <h1 id="guide-title">{title}</h1>
        <nav className="guide-nav">
          {sections.map((section) => (
            <a key={section.id} href={`#${section.id}`} onClick={(event) => (event.preventDefault(), document.getElementById(section.id)?.scrollIntoView({ behavior: "smooth" }))}>
              {section.text.split(/[：:]/)[0]}
            </a>
          ))}
        </nav>
        {blocks.map((block, index) => {
          switch (block.kind) {
            case "title":
              return null;
            case "section":
              return <h2 key={index} id={block.id}>{inline(block.text)}</h2>;
            case "sub":
              return <h3 key={index}>{inline(block.text)}</h3>;
            case "para":
              return <p key={index}>{inline(block.text)}</p>;
            case "note":
              return <p key={index} className="guide-note">{inline(block.text)}</p>;
            case "list":
              return (
                <ul key={index}>
                  {block.items.map((item, i) => (
                    <li key={i}>
                      {inline(item.text)}
                      {item.children.length > 0 && <ul>{item.children.map((child, j) => <li key={j}>{inline(child)}</li>)}</ul>}
                    </li>
                  ))}
                </ul>
              );
            case "shot": {
              const src = failed.has(block.key) ? undefined : (pickGuideImage(shots, block.key, lang) ?? undefined);
              if (!src) return null;
              const phone = Number(block.key.replace(/\D/g, "")) >= PHONE_FROM;
              return (
                <figure key={index} className={phone ? "guide-shot guide-shot-phone" : "guide-shot"}>
                  {/* 手機上桌面截圖縮得太小：點了另開原圖，可以放大看 */}
                  <a href={src} target="_blank" rel="noopener noreferrer">
                    <img src={src} alt="" loading="lazy" onError={() => setFailed((prev) => new Set(prev).add(block.key))} />
                  </a>
                </figure>
              );
            }
          }
        })}
      </article>
    </div>
  );
}
