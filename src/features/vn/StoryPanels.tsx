// 結局一覽與章節選擇（vn-mode 包 5 §2.8、§2.9）：蓋在播放器上的暗幕面板，工具列 ⋯ 與完結畫面共用。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import type { VnEndings } from "../../shared/contracts/backend-contracts";
import "./vn-story.css";

/** 結局表＋本桌任一分支達成過的（`vn_endings`）；`key` 變了就重讀 */
export function useEndings(worldId: string, key: string) {
  const [endings, setEndings] = useState<VnEndings | null>(null);
  useEffect(() => {
    let live = true;
    invoke<VnEndings>("vn_endings", { worldId })
      .then((value) => live && setEndings(value))
      .catch(() => live && setEndings(null));
    return () => {
      live = false;
    };
  }, [worldId, key]);
  return endings;
}

export const reachedCount = (endings: VnEndings) => endings.endings.filter((ending) => ending.reached).length;

export function Panel({ label, children, onClose }: { label: string; children: React.ReactNode; onClose: () => void }) {
  return (
    <div className="vn-story-panel" role="dialog" aria-label={label}>
      <p className="vn-chapter-heading">{label}</p>
      {children}
      <div className="vn-chapter-actions">
        <button type="button" className="btn" onClick={onClose}>
          {t("vnChapterClose")}
        </button>
      </div>
    </div>
  );
}

/** 已達成顯示標題（`default` 沒標題時顯示「完」），未達成「???」；劇透鎖住時也一樣 */
export function EndingsPanel({ worldId, onClose }: { worldId: string; onClose: () => void }) {
  const endings = useEndings(worldId, "");
  return (
    <Panel label={t("vnEndingsList")} onClose={onClose}>
      {endings && (
        <>
          <p className="vn-chapter-note">
            {t("vnEndingsReached", { x: reachedCount(endings), y: endings.endings.length })}
          </p>
          <ol className="vn-story-list">
            {endings.endings.map((ending) => (
              <li key={ending.id} className={ending.reached ? undefined : "vn-unreached"}>
                {ending.reached ? ending.title || t("vnTheEnd") : "???"}
              </li>
            ))}
          </ol>
        </>
      )}
    </Panel>
  );
}

/** 第 1 章到目前這章；選了就交給 `onPick` */
export function ChapterPanel({
  label = t("vnReplayPick"),
  chapter,
  disabled,
  onPick,
  onClose,
}: {
  /** 重播桌換成「從哪一章看起」 */
  label?: string;
  chapter: number;
  disabled: boolean;
  onPick: (chapter: number) => void;
  onClose: () => void;
}) {
  return (
    <Panel label={label} onClose={onClose}>
      <div className="vn-story-list">
        {Array.from({ length: chapter }, (_, index) => index + 1).map((n) => (
          <button key={n} type="button" className="btn" disabled={disabled} onClick={() => onPick(n)}>
            {t("vnChapterStart", { n })}
          </button>
        ))}
      </div>
    </Panel>
  );
}
