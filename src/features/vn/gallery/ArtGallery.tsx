import { ArtFlowNotice } from "../art-flow/ArtFlowNotice";
// 美術與音樂（plans/vn-setup-wizard.md §4、plans/vn-credits.md）：標題畫面與遊戲選單共用的面板，分頁「立繪｜背景｜音樂」。
// 縮圖點開燈箱檢視；重播桌只能看（立繪不開編輯、背景不出生成與上傳鈕）。音樂頁重播桌照常列。
import { useState } from "react";
import { t } from "../../../i18n";
import { Panel } from "../StoryPanels";
import type { useBackgrounds } from "../useBackgrounds";
import { ArtViewer, type Viewing } from "./ArtViewer";
import { BackgroundGallery } from "./BackgroundGallery";
import { MusicCredits } from "./MusicCredits";
import { SpriteGallery } from "./SpriteGallery";
import "./vn-gallery.css";

const TABS = [
  ["sprites", "vnGalleryTabSprites"],
  ["backgrounds", "vnGalleryTabBackgrounds"],
  ["music", "vnGalleryTabMusic"],
] as const;

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
  const [tab, setTab] = useState<(typeof TABS)[number][0]>("sprites");
  const [viewing, setViewing] = useState<Viewing | null>(null);
  return (
    <Panel label={t("vnGallery")} onClose={onClose} className="vn-gallery">
      {!replay && <ArtFlowNotice worldId={worldId} />}
      <div className="vn-gallery-tabs" role="tablist">
        {TABS.map(([key, label]) => (
          <button
            key={key}
            type="button"
            role="tab"
            className="btn btn-sm"
            aria-selected={tab === key}
            onClick={() => setTab(key)}
          >
            {t(label)}
          </button>
        ))}
      </div>
      {replay && <p className="vn-chapter-note">{t("vnGalleryReadOnly")}</p>}
      <div className="vn-gallery-body">
        {tab === "sprites" ? (
          <SpriteGallery worldId={worldId} images={images} readOnly={replay} onView={setViewing} />
        ) : tab === "backgrounds" ? (
          <BackgroundGallery worldId={worldId} backgrounds={backgrounds} readOnly={replay} onView={setViewing} />
        ) : (
          <MusicCredits worldId={worldId} />
        )}
      </div>
      {viewing && <ArtViewer viewing={viewing} onClose={() => setViewing(null)} />}
    </Panel>
  );
}
