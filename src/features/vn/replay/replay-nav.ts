// 重播桌的導覽（vn-mode 包 8 §2.6）：作品桌不生成新內容，所有「去別處」都只換目前幕（`vn_replay_goto`），不分岔。
// 幕的前後關係照後端 `data/scene/branches.rs`：換幕子幕＝`parent` 是它且不是分岔；劇情前一幕＝分岔走來源、換幕走 parent。
import { invoke } from "@tauri-apps/api/core";
import type { SceneLabel, VnChoiceBranch } from "../../../shared/contracts/backend-contracts";
import type { VnAdvance } from "../../play/scene/useSceneActions";
import type { VnEntry } from "../useBranchOps";
import type { EntryAt } from "../vn-cells";

type Labels = Record<string, SceneLabel>;

/** 換幕來的子幕（章末接著讀）；沒有＝作者在這幕之後沒再換幕 */
export function nextScene(labels: Labels, scene: number): number | null {
  const found = Object.entries(labels).find(([, label]) => !label.forked && label.parent === scene);
  return found ? Number(found[0]) : null;
}

/** 劇情上的前一幕（「回上一個選項」跨幕用） */
export function previousScene(labels: Labels, scene: number): number | null {
  const label = labels[String(scene)];
  if (!label) return null;
  return label.forked && label.source !== undefined ? label.source : label.parent;
}

const START: EntryAt = { kind: "start" };

export function replayNav({
  worldId,
  scene,
  labels,
  goto,
  jump,
  setEntry,
  onBranch,
}: {
  worldId: string;
  scene: number;
  labels: Labels;
  /** `useBranchOps().goto`：鎖、重進、同幕直接跳格都在那裡 */
  goto: (work: () => Promise<number>, at: EntryAt) => Promise<void>;
  jump: (at: EntryAt) => void;
  setEntry: (entry: VnEntry | null) => void;
  onBranch: (run: () => Promise<unknown>) => Promise<VnAdvance>;
}) {
  const next = nextScene(labels, scene);
  const previous = previousScene(labels, scene);
  const to = (target: number) => () => invoke<number>("vn_replay_goto", { worldId, scene: target });
  return {
    next,
    /** 標題「開始」：回第 0 幕幕首，不問確認 */
    start: () => goto(to(0), START),
    /** 章節選擇：後端換到章頭幕（`vn_replay_chapter` 重播桌不分岔） */
    chapter: (chapter: number) => goto(() => invoke<number>("vn_replay_chapter", { worldId, chapter }), START),
    /** 選項點上作者走過的別條：跳到選了那句的幕、停在那則玩家句之後 */
    choose: (branch: VnChoiceBranch) =>
      goto(to(branch.scene), branch.answer_event_id ? { kind: "after", eventKey: branch.answer_event_id } : START),
    /** 幕末沒有章末格的換幕（容量換幕）：直接到子幕幕首 */
    forward: () => (next === null ? Promise.resolve() : goto(to(next), START)),
    /** 章結束流程的換幕（字卡佔著鎖，不能再走 goto 的鎖）：「第 N 章 完」先停一下再到子幕幕首，分階段回報 */
    advance: async (): Promise<VnAdvance> => {
      if (next === null) return { stage: "failed", message: null };
      await new Promise((resolve) => setTimeout(resolve, 1000));
      setEntry({ from: scene, at: START });
      const result = await onBranch(to(next));
      if (result.stage === "failed") setEntry(null);
      return result;
    },
    /** 回上一個選項：本幕最後一個選過的選項格；本幕沒有就退劇情前一幕的（只退一幕） */
    backToChoice: (here: boolean) => {
      if (here) return jump({ kind: "lastChoice" });
      if (previous !== null) void goto(to(previous), { kind: "lastChoice" });
    },
    canBack: (here: boolean) => here || previous !== null,
  };
}
