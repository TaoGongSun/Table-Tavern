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

/** 已達成 x／y 與結局一覽只算正式結局：保底結局（`default`）不算〔作者裁決 2026-11-02〕 */
export const countedEndings = (endings: VnEndings) => endings.endings.filter((ending) => ending.id !== "default");

/** 「已達成 x／y」；沒有正式結局（沒有聖經）回 null 不顯示 */
export function reachedText(endings: VnEndings): string | null {
  const counted = countedEndings(endings);
  if (counted.length === 0) return null;
  const x = counted.filter((ending) => ending.reached).length;
  return t("vnEndingsReached", { x, y: counted.length });
}

export function Panel({
  label,
  children,
  onClose,
  className,
}: {
  label: string;
  children: React.ReactNode;
  onClose: () => void;
  className?: string;
}) {
  return (
    <div className={`vn-story-panel${className ? ` ${className}` : ""}`} role="dialog" aria-label={label}>
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

/** 已達成顯示標題（沒標題時顯示「完」），未達成「???」；劇透鎖住時也一樣 */
export function EndingsPanel({ worldId, onClose }: { worldId: string; onClose: () => void }) {
  const endings = useEndings(worldId, "");
  return (
    <Panel label={t("vnEndingsList")} onClose={onClose}>
      {endings && (
        <>
          {reachedText(endings) && <p className="vn-chapter-note">{reachedText(endings)}</p>}
          <ol className="vn-story-list">
            {countedEndings(endings).map((ending) => (
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
