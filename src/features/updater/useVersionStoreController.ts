import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "../../shared/contracts/backend-contracts";
import { previousRow, rollbackFlowBusy, type RollbackCheck } from "./version-center";

export type VersionRow = {
  version: string;
  size: number;
  format_version: number | null;
  usable: boolean;
  /** 可用、而且比目前舊（後端與回退同一支資格判斷）。 */
  eligible: boolean;
  current: boolean;
  previous: boolean;
};

export type VersionList = {
  versions: VersionRow[];
  total_bytes: number;
};

export type PreviewWorld = { id: string; name: string };

export type RollbackPreview = {
  will_be_readonly: PreviewWorld[];
  maybe_readonly: PreviewWorld[];
  scan_failed: boolean;
};

export type BackupKind = "pre" | "newer";

export type BackupRow = {
  world_id: string;
  name: string;
  kind: BackupKind;
  size: number;
  format_version: number | null;
  deletable: boolean;
  needs_repair: boolean;
  /** 該份備份目錄的絕對路徑。 */
  directory: string;
};

export type BackupList = {
  backups: BackupRow[];
  total_bytes: number;
};

export type RollbackPhase =
  | { kind: "idle" }
  | { kind: "preview"; version: string }
  | { kind: "waiting"; version: string }
  | { kind: "installing"; version: string }
  | { kind: "error"; version: string; message: string };

type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const defaultInvoke: InvokeFn = (command, args) => invoke(command, args);
const notBlocked = () => false;

export type VersionStoreControllerOptions = {
  /** 啟動後整理（update_post_launch）完成。到這一步才拉版本庫與桌備份。 */
  initialLoadReady: boolean;
  responding: boolean;
  /** 更新流程進行中：不能開始回退。 */
  isBlocked?: () => boolean;
  /** 預覽之後的確認窗。回 true 才往下回退；目標不合格時只顯示原因、一律回 false。 */
  askRollback: (version: string, check: RollbackCheck) => Promise<boolean>;
  /** 回退失敗後重讀的設定（後端可能已把目前版本寫進略過鍵）。 */
  onConfig: (config: AppConfig) => void;
  invokeImpl?: InvokeFn;
};

/**
 * 版本清單、回退預覽與確認、回退、刪版、桌備份。不畫畫面。
 * 一鍵與清單的回退都先跑預覽、問過確認窗；AI 回應中只進 waiting，不呼叫 stopResponse。
 * 一鍵上一版只在清單上有同時是 `previous` 與 `eligible` 的那一列時才開始。
 */
export function useVersionStoreController({
  initialLoadReady,
  responding,
  isBlocked = notBlocked,
  askRollback,
  onConfig,
  invokeImpl = defaultInvoke,
}: VersionStoreControllerOptions) {
  const [versions, setVersions] = useState<VersionList | null>(null);
  const [backups, setBackups] = useState<BackupList | null>(null);
  const [phase, setPhaseState] = useState<RollbackPhase>({ kind: "idle" });
  const [loadError, setLoadError] = useState<string | null>(null);
  const phaseRef = useRef(phase);
  const setPhase = useCallback((next: RollbackPhase) => {
    phaseRef.current = next;
    setPhaseState(next);
  }, []);
  const respondingRef = useRef(responding);
  respondingRef.current = responding;
  const versionsRef = useRef(versions);
  versionsRef.current = versions;
  const blockedRef = useRef(isBlocked);
  blockedRef.current = isBlocked;
  const askRef = useRef(askRollback);
  askRef.current = askRollback;

  /** 失敗留在 loadError，不當成沒有版本：原本的清單照留。 */
  const refresh = useCallback(async () => {
    try {
      const [nextVersions, nextBackups] = await Promise.all([
        invokeImpl("list_versions"),
        invokeImpl("list_world_backups"),
      ]);
      setVersions(nextVersions as VersionList);
      setBackups(nextBackups as BackupList);
      setLoadError(null);
    } catch (reason) {
      setLoadError(String(reason));
    }
  }, [invokeImpl]);

  useEffect(() => {
    if (!initialLoadReady) return;
    void refresh();
  }, [initialLoadReady, refresh]);

  const runRollback = useCallback(
    async (version: string) => {
      setPhase({ kind: "installing", version });
      try {
        await invokeImpl("rollback_install", { version });
      } catch (reason) {
        // 略過鍵在開閘前就寫了。失敗時閘已放下，重讀設定，畫面才看得到目前這版被標成略過。
        try {
          onConfig((await invokeImpl("read_config")) as AppConfig);
        } catch {
          // 讀不回設定仍顯示回退錯誤。
        }
        setPhase({ kind: "error", version, message: String(reason) });
      }
    },
    [invokeImpl, onConfig, setPhase],
  );

  useEffect(() => {
    if (responding) return;
    if (phase.kind !== "waiting") return;
    void runRollback(phase.version);
  }, [responding, phase, runRollback]);

  const startRollback = useCallback(
    async (version: string) => {
      if (rollbackFlowBusy(phaseRef.current) || blockedRef.current()) return;
      setPhase({ kind: "preview", version });
      let check: RollbackCheck;
      try {
        const preview = (await invokeImpl("rollback_preview", { version })) as RollbackPreview;
        check = { kind: "preview", preview };
      } catch (reason) {
        check = { kind: "invalid", message: String(reason) };
      }
      let proceed = false;
      try {
        proceed = (await askRef.current(version, check)) && check.kind === "preview";
      } catch {
        proceed = false;
      }
      if (!proceed) {
        setPhase({ kind: "idle" });
        return;
      }
      if (respondingRef.current) {
        setPhase({ kind: "waiting", version });
        return;
      }
      await runRollback(version);
    },
    [invokeImpl, runRollback, setPhase],
  );

  const startRollbackPrevious = useCallback(async () => {
    const row = previousRow(versionsRef.current?.versions);
    if (!row) return;
    await startRollback(row.version);
  }, [startRollback]);

  const cancelWait = useCallback(() => {
    if (phaseRef.current.kind === "waiting") setPhase({ kind: "idle" });
  }, [setPhase]);

  const deleteVersion = useCallback(
    async (version: string) => {
      try {
        await invokeImpl("delete_version", { version });
      } finally {
        await refresh();
      }
    },
    [invokeImpl, refresh],
  );

  const deleteBackup = useCallback(
    async (worldId: string, kind: BackupKind) => {
      try {
        await invokeImpl("delete_world_backup", { worldId, kind });
      } finally {
        await refresh();
      }
    },
    [invokeImpl, refresh],
  );

  return {
    versions,
    backups,
    phase,
    loadError,
    isFlowBusy: () => rollbackFlowBusy(phaseRef.current),
    refresh,
    startRollback,
    startRollbackPrevious,
    cancelWait,
    deleteVersion,
    deleteBackup,
  };
}

export type VersionStoreController = ReturnType<typeof useVersionStoreController>;
