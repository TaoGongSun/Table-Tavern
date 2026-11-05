// 標題畫面（vn-mode 包 6 §2.8）：從大廳進 VN 桌時先到這裡。沒讀過＝開始當主鈕（有存檔加讀檔）；讀過＝繼續、開始、讀檔；
// 都有結局一覽／美術一覽／設定／回大廳，面板疊在標題上；
// 換幕、分岔、讀檔等重進本桌不出。背景用封面（VN 卡匯入的桌才有），沒有就用目前的背景圖。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { useArtProgress } from "./wizard/useArtProgress";
import "./vn-sprites.css";
import "./vn-story.css";

export function TitleScreen({
  worldId,
  tableName,
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
  tableName: string;
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
  const { progress: art, retry } = useArtProgress(worldId);
  const image = cover ?? backdrop;
  // 立繪進度、失敗、背景各自獨立；行高一直保留，消失時按鈕不跳
  const preparing = art !== null && art.ready + art.failed < art.total;
  return (
    <div className="vn-title-screen" role="dialog" aria-label={tableName}>
      {image && <img className="vn-title-art" src={image} alt="" />}
      <div className="vn-title-body">
        <h1 className="vn-title-name">{tableName}</h1>
        <p className="vn-title-art-progress">
          {preparing && <span className="vn-pending">{t("vnArtPreparing", { n: art.ready, total: art.total })}</span>}
          {art !== null && art.failed > 0 && (
            <span className="vn-title-art-failed">
              {t("vnArtFailed", { n: art.failed })}
              <button type="button" className="btn btn-ghost btn-sm" onClick={retry}>
                {t("vnArtRetry")}
              </button>
            </span>
          )}
          {!image && <span className="vn-pending">{t("vnArtBackdropPending")}</span>}
        </p>
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
