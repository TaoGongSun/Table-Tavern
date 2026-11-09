// 過去的選項：重播統一標示已玩狀態；自己遊玩倒回時標示上次選的那句。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, type ReactNode } from "react";
import { t } from "../../i18n";
import type { VnChoiceBranch } from "../../shared/contracts/backend-contracts";

export function PastChoicePanel({
  worldId,
  eventId,
  options,
  answered,
  disabled,
  onCurrent,
  onSwitch,
  onFork,
  deadEnd,
}: {
  worldId: string;
  eventId: string;
  options: string[];
  /** 本分支選的那句；null＝重播桌停在還沒選的選項點 */
  answered: string | null;
  disabled: boolean;
  onCurrent: () => void;
  onSwitch: (branch: VnChoiceBranch) => void;
  /** 沒給＝重播桌 */
  onFork?: (option: string) => void;
  /** 重播桌「此路作者未完成」：已選的照給就顯示；未選的等分支查回來、一條都沒走過才顯示 */
  deadEnd?: ReactNode;
}) {
  const replay = !onFork;
  // 查詢結果綁定這個選項點：還沒查到（或查詢屬於別的選項點）之前，除了自己上次選的那句都不能按——
  // 否則玩過的路線會被當成沒走過，重開一條分支並花一輪生成
  const key = `${worldId}\u0000${eventId}`;
  const [query, setQuery] = useState<{ key: string; branches: VnChoiceBranch[] | null; failed: boolean }>({
    key: "",
    branches: null,
    failed: false,
  });
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let live = true;
    setQuery({ key, branches: null, failed: false });
    invoke<VnChoiceBranch[]>("vn_choice_branches", { worldId, eventId })
      .then((value) => live && setQuery({ key, branches: value ?? [], failed: false }))
      .catch(() => live && setQuery({ key, branches: null, failed: true }));
    return () => {
      live = false;
    };
  }, [key, worldId, eventId, attempt]);
  const ready = query.key === key && query.branches !== null;
  const branches = ready ? (query.branches ?? []) : [];
  const played = new Map(branches.map((branch) => [branch.option, branch]));
  const others = [...played.values()].filter((branch) => !options.includes(branch.option));
  const tag = (option: string, branch?: VnChoiceBranch) => {
    const label = !replay && option === answered ? t("vnChoiceLast") : t("vnChoicePlayed");
    return branch?.ended ? `${label}・${t("vnChoiceEnded")}` : replay ? t("vnChoicePlayedUnfinished") : label;
  };
  return (
    <div className="vn-choices vn-choices-past">
      {options.map((option, index) => {
        const branch = played.get(option);
        const current = option === answered;
        const unwalked = replay && ready && !branch;
        return (
          <button
            key={`${index}:${option}`}
            type="button"
            className={`btn vn-choice${unwalked ? " vn-choice-unwalked" : ""}`}
            disabled={disabled || (!current && !ready) || unwalked}
            onClick={() => (current ? onCurrent() : branch ? onSwitch(branch) : onFork?.(option))}
          >
            {option}
            {(branch || (current && !replay)) && <span className="vn-choice-tag">{tag(option, branch)}</span>}
            {unwalked && <span className="vn-choice-tag">{t("vnWorkNotWalked")}</span>}
          </button>
        );
      })}
      {others.map((branch) => (
        <button
          key={`other:${branch.option}`}
          type="button"
          className="btn btn-ghost vn-choice"
          disabled={disabled}
          onClick={() => onSwitch(branch)}
        >
          {branch.option}
          <span className="vn-choice-tag">{tag(branch.option, branch)}</span>
        </button>
      ))}
      {(answered !== null || (ready && played.size === 0)) && deadEnd}
      {query.key === key && query.failed && (
        <p className="vn-choice-error" role="alert">
          {t("vnChoiceBranchesFailed")}{" "}
          <button type="button" className="btn" onClick={() => setAttempt((n) => n + 1)}>
            {t("vnBranchReenterRetry")}
          </button>
        </p>
      )}
    </div>
  );
}
