// 標題畫面（vn-mode 包 6 §2.8）：從大廳進 VN 桌時先到這裡。開始／繼續／讀檔／結局一覽／設定／回大廳；
// 換幕、分岔、讀檔等重進本桌不出。背景用封面（VN 卡匯入的桌才有），沒有就用目前的背景圖。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import "./vn-story.css";

export function TitleScreen({
  worldId,
  tableName,
  backdrop,
  disabled,
  onStart,
  onContinue,
  onLoad,
  onEndings,
  onSettings,
  onLobby,
}: {
  worldId: string;
  tableName: string;
  backdrop: string | undefined;
  disabled: boolean;
  onStart: () => void;
  onContinue: () => void;
  onLoad: () => void;
  onEndings: () => void;
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
  const image = cover ?? backdrop;
  return (
    <div className="vn-title-screen" role="dialog" aria-label={tableName}>
      {image && <img className="vn-title-art" src={image} alt="" />}
      <div className="vn-title-body">
        <h1 className="vn-title-name">{tableName}</h1>
        <div className="vn-title-menu">
          <button type="button" className="btn btn-primary" disabled={disabled} onClick={onContinue}>
            {t("vnTitleContinue")}
          </button>
          <button type="button" className="btn" disabled={disabled} onClick={onStart}>
            {t("vnTitleStart")}
          </button>
          <button type="button" className="btn" disabled={disabled} onClick={onLoad}>
            {t("vnTitleLoad")}
          </button>
          <button type="button" className="btn" onClick={onEndings}>
            {t("vnEndingsList")}
          </button>
          <button type="button" className="btn" onClick={onSettings}>
            {t("vnTitleSettings")}
          </button>
          <button type="button" className="btn btn-ghost" onClick={onLobby}>
            {t("backToLobby")}
          </button>
        </div>
      </div>
    </div>
  );
}
