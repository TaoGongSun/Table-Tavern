// 過去的選項面板（vn-mode 包 6 §2.5）：回溯停在已選過的選項格時出現。本分支選的標「目前路線」（按了往前讀）；
// 別條分支玩過的標「已玩過」（按了切過去）；沒走過的按了從這裡分岔並送出。別條換一組後選的另列。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
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
}: {
  worldId: string;
  eventId: string;
  options: string[];
  /** 本分支選的那句 */
  answered: string;
  disabled: boolean;
  onCurrent: () => void;
  onSwitch: (tip: number) => void;
  onFork: (option: string) => void;
}) {
  // 查詢結果綁定這個選項點：還沒查到（或查詢屬於別的選項點）之前，除了「目前路線」都不能按——
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
  const played = new Map(branches.filter((branch) => branch.option !== answered).map((branch) => [branch.option, branch]));
  const others = [...played.values()].filter((branch) => !options.includes(branch.option));
  const tag = (branch: VnChoiceBranch) =>
    branch.ended ? `${t("vnChoicePlayed")} · ${t("vnChoiceEnded")}` : t("vnChoicePlayed");
  return (
    <div className="vn-choices vn-choices-past">
      {options.map((option, index) => {
        const branch = played.get(option);
        const current = option === answered;
        return (
          <button
            key={`${index}:${option}`}
            type="button"
            className={`btn vn-choice${current ? " vn-choice-current" : ""}`}
            disabled={disabled || (!current && !ready)}
            onClick={() => (current ? onCurrent() : branch ? onSwitch(branch.tip) : onFork(option))}
          >
            {option}
            {current && <span className="vn-choice-tag">{t("vnChoiceCurrent")}</span>}
            {branch && <span className="vn-choice-tag">{tag(branch)}</span>}
          </button>
        );
      })}
      {others.map((branch) => (
        <button
          key={`other:${branch.option}`}
          type="button"
          className="btn btn-ghost vn-choice"
          disabled={disabled}
          onClick={() => onSwitch(branch.tip)}
        >
          {t("vnChoiceOtherPlayed", { option: branch.option })}
          <span className="vn-choice-tag">{tag(branch)}</span>
        </button>
      ))}
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
