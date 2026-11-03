// 選項點分岔、切換分支與讀檔（vn-mode 包 6 §2.3、§2.4、§2.7）：分支動作 → 重進本桌 → 播放器換到新幕後才送出選項。
// 三階段各自收尾：分支動作失敗＝什麼都沒發布；已提交但重進失敗＝只重試重進，絕不再分岔；重進前不送任何字
// （前端的幕號還是舊幕，送出會落到舊分支——後端另有幕號守門）。送出失敗時新分支停在可重按的選項面板。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { VnLoaded } from "../../shared/contracts/backend-contracts";
import type { VnAdvance } from "../play/scene/useSceneActions";

/** 進新幕的指定位置：from＝離開的幕號（換到別的幕才套用）；key＝停在這一格，null＝最後一格 */
export interface VnEntry {
  from: number;
  key: string | null;
}

export function useBranchOps({
  worldId,
  scene,
  formalScene,
  live,
  onBranch,
  onReenter,
  runLocked,
  submit,
  setEntry,
  jump,
}: {
  worldId: string;
  /** App 的目前幕號（分支動作開始時記下，換成別的才算換手） */
  scene: number;
  /** 播放器正式格所屬的幕 */
  formalScene: number | null;
  /** 新幕有還沒選的選項格 */
  live: boolean;
  onBranch: (run: () => Promise<unknown>) => Promise<VnAdvance>;
  onReenter: () => Promise<boolean>;
  runLocked: (work: () => Promise<void>) => Promise<void>;
  submit: (option: string) => void;
  /** 進新幕的位置；null＝照一般進桌位置。播放器在新幕定位後自己清掉 */
  setEntry: (entry: VnEntry | null) => void;
  /** 讀檔落在目前這幕：直接跳格 */
  jump: (key: string) => void;
}) {
  /** 已重進、等新幕的選項面板出來就送出 */
  const [pending, setPending] = useState<{ from: number; option: string } | null>(null);
  /** 已提交、重進失敗：重試只重進（option＝之後要送的選項） */
  const [reenter, setReenter] = useState<{ from: number; option: string | null } | null>(null);
  useEffect(() => {
    if (!pending || formalScene === null || formalScene === pending.from || !live) return;
    setPending(null);
    submit(pending.option);
  }, [pending, formalScene, live, submit]);

  /** `land`：分支動作回來後進新幕停哪（null＝最後一格；undefined＝留在原幕、不設進桌位置） */
  const run = (work: () => Promise<unknown>, option: string | null, land: () => string | null | undefined = () => null) =>
    runLocked(async () => {
      const from = scene;
      setEntry({ from, key: null });
      const result = await onBranch(work);
      const key = land();
      setEntry(result.stage === "failed" || key === undefined ? null : { from, key });
      if (result.stage === "committed") {
        setReenter({ from, option });
      } else if (option !== null) {
        setPending({ from, option });
      }
    });

  return {
    reenter,
    /** 從過去的選項點分岔並送出沒走過的選項 */
    forkAt: (eventId: string, option: string) =>
      run(() => invoke<number>("vn_fork_choice", { worldId, eventId }), option),
    /** 切到別條玩過的分支尾幕 */
    switchTo: (tip: number) => run(() => invoke<number>("vn_switch_branch", { worldId, scene: tip }), null),
    /** 讀檔：目前幕只跳格；別條分支或分岔出來的幕重進後停在存檔那格 */
    load: (id: string) => {
      let loaded: VnLoaded | null = null;
      return run(
        async () => {
          loaded = await invoke<VnLoaded>("vn_save_load", { worldId, id });
          if (loaded.scene === scene) jump(`${loaded.event_id}#${loaded.cell}`);
        },
        null,
        () => {
          const at: VnLoaded | null = loaded;
          if (!at) return null;
          return at.scene === scene ? undefined : `${at.event_id}#${at.cell}`;
        },
      );
    },
    retry: () =>
      runLocked(async () => {
        const now = reenter;
        if (!now || !(await onReenter())) return;
        setReenter(null);
        if (now.option !== null) setPending({ from: now.from, option: now.option });
      }),
  };
}
