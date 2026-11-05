// 美術一覽（plans/vn-setup-wizard.md §4）：標題畫面與遊戲選單共用的面板，分頁「立繪｜背景」。
// 縮圖點開燈箱檢視；重播桌只能看（立繪不開編輯、背景不出生成與上傳鈕）。
import { useState } from "react";
import { t } from "../../../i18n";
import { Panel } from "../StoryPanels";
import type { useBackgrounds } from "../useBackgrounds";
import { ArtViewer, type Viewing } from "./ArtViewer";
import { BackgroundGallery } from "./BackgroundGallery";
import { SpriteGallery } from "./SpriteGallery";
import "./vn-gallery.css";

export function ArtGallery({
  worldId,
  images,
  backgrounds,
  replay,
  onClose,
}: {
  worldId: string;
  /** 角色 id → 角色卡原圖（還沒有定稿時的縮圖） */
  images: Record<string, string>;
  /** 舞台那份背景狀態：一覽的生成、上傳完成時舞台跟著重讀 */
  backgrounds: ReturnType<typeof useBackgrounds>;
  replay: boolean;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<"sprites" | "backgrounds">("sprites");
  const [viewing, setViewing] = useState<Viewing | null>(null);
  return (
    <Panel label={t("vnGallery")} onClose={onClose} className="vn-gallery">
      <div className="vn-gallery-tabs" role="tablist">
        {(["sprites", "backgrounds"] as const).map((key) => (
          <button
            key={key}
            type="button"
            role="tab"
            className="btn btn-sm"
            aria-selected={tab === key}
            onClick={() => setTab(key)}
          >
            {t(key === "sprites" ? "vnGalleryTabSprites" : "vnGalleryTabBackgrounds")}
          </button>
        ))}
      </div>
      {replay && <p className="vn-chapter-note">{t("vnGalleryReadOnly")}</p>}
      <div className="vn-gallery-body">
        {tab === "sprites" ? (
          <SpriteGallery worldId={worldId} images={images} readOnly={replay} onView={setViewing} />
        ) : (
          <BackgroundGallery worldId={worldId} backgrounds={backgrounds} readOnly={replay} onView={setViewing} />
        )}
      </div>
      {viewing && <ArtViewer viewing={viewing} onClose={() => setViewing(null)} />}
    </Panel>
  );
}
