// 故事頁頂端的三鍵狀態列：只看已落段的 story，串流中不換；手機平時一行、點一下展開。
import { useState } from "react";
import type { SceneStatus } from "../../../server/core/types";
import { t, type MsgKey } from "../../i18n";

const KEYS = ["time", "place", "present"] as const;
const LABELS: Record<(typeof KEYS)[number], MsgKey> = {
  time: "statusTime",
  place: "statusPlace",
  present: "statusPresent",
};

/** 由舊到新合併：這段有那個鍵就換（含空字串），沒有就留上一筆 */
export function mergedSceneStatus(parts: readonly { state?: SceneStatus }[]): SceneStatus {
  const out: SceneStatus = {};
  for (const part of parts) {
    const state = part.state;
    if (!state) continue;
    for (const key of KEYS) if (state[key] !== undefined) out[key] = state[key];
  }
  return out;
}

/** 缺的鍵、空字串、只剩空白都不列 */
export function visibleSceneStatus(parts: readonly { state?: SceneStatus }[]) {
  const status = mergedSceneStatus(parts);
  return KEYS.flatMap((key) => {
    const text = status[key]?.trim();
    return text ? [{ key, label: t(LABELS[key]), text }] : [];
  });
}

export function SceneStatusLine({ parts }: { parts: readonly { state?: SceneStatus }[] }) {
  const items = visibleSceneStatus(parts);
  const [open, setOpen] = useState(false);
  if (!items.length) return null;
  // 手機平時一行，點一下展開到三行（桌面版一律三行，App.css 只在 .party-phone 收）
  return (
    <div
      className={open ? "scene-status open" : "scene-status"}
      data-testid="scene-status"
      onClick={() => setOpen((o) => !o)}
    >
      <div className="scene-status-text">
        {items.map((item, index) => (
          <span key={item.key} className="scene-status-item" title={`${item.label} ${item.text}`}>
            {index > 0 && " · "}
            <span className="k">{item.label} </span>
            {item.text}
          </span>
        ))}
      </div>
    </div>
  );
}
