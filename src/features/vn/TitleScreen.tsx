import { ArtFlowNotice } from "./art-flow/ArtFlowNotice";
// 標題畫面（vn-mode 包 6 §2.8）：從大廳進 VN 桌時先到這裡。沒讀過＝開始當主鈕（有存檔加讀檔）；讀過＝繼續、開始、讀檔；
// 都有結局一覽／美術一覽／設定／回大廳，面板疊在標題上；
// 換幕、分岔、讀檔等重進本桌不出。背景用封面（VN 卡匯入的桌才有），沒有就用目前的背景圖。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { useArtProgress } from "./art-flow/useArtProgress";
import "./vn-sprites.css";
import "./vn-story.css";

export function TitleScreen({
  worldId,
  replay = false,
  tableName,
  tagline,
  backdrop,
  disabled,
  progress,
  onStart,
  onContinue,
  onLoad,
  onEndings,
  onGallery,
  onSettings,
  onLobby,
}: {
  worldId: string;
  replay?: boolean;
  tableName: string;
  /** 主角一句話；空的不顯示 */
  tagline?: string;
  backdrop: string | undefined;
  disabled: boolean;
  /** loading＝進度還沒讀回來（先不出選單，免得主鈕閃一下）；fresh＝沒讀過也沒存檔；saved＝沒讀過但有存檔；resume＝讀過 */
  progress: "loading" | "fresh" | "saved" | "resume";
  onStart: () => void;
  onContinue: () => void;
  onLoad: () => void;
  onEndings: () => void;
  onGallery: () => void;
  onSettings: () => void;
  onLobby: () => void;
}) {
  const [cover, setCover] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    invoke<string | null>("vn_cover", { worldId })
      .then((value) => live && setCover(value ?? null))
      .catch(() => live && setCover(null));
    return () => {
      live = false;
    };
  }, [worldId]);
  useArtProgress(worldId);
  const image = cover ?? backdrop;
  return (
    <div className="vn-title-screen" role="dialog" aria-label={tableName}>
      {image && <img className="vn-title-art" src={image} alt="" />}
      <div className="vn-title-body">
        {!replay && <ArtFlowNotice worldId={worldId} compact onOpenGallery={onGallery} />}
        <h1 className="vn-title-name">{tableName}</h1>
        {tagline && <p className="vn-title-tagline">{tagline}</p>}
        {progress !== "loading" && (
          <div className="vn-title-menu">
            {progress === "resume" && (
              <button type="button" className="btn btn-primary" disabled={disabled} onClick={onContinue}>
                {t("vnTitleContinue")}
              </button>
            )}
            <button
              type="button"
              className={progress === "resume" ? "btn" : "btn btn-primary"}
              disabled={disabled}
              onClick={onStart}
            >
              {t("vnTitleStart")}
            </button>
            {progress !== "fresh" && (
              <button type="button" className="btn" disabled={disabled} onClick={onLoad}>
                {t("vnTitleLoad")}
              </button>
            )}
            <button type="button" className="btn" onClick={onEndings}>
              {t("vnEndingsList")}
            </button>
            <button type="button" className="btn" onClick={onGallery}>
              {t("vnGallery")}
            </button>
            <button type="button" className="btn" onClick={onSettings}>
              {t("vnTitleSettings")}
            </button>
            <button type="button" className="btn btn-ghost" onClick={onLobby}>
              {t("backToLobby")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
