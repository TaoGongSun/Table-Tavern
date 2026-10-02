import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type VersionRow = {
  version: string;
  size: number;
  format_version: number | null;
  usable: boolean;
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
};

export type BackupList = {
  backups: BackupRow[];
  total_bytes: number;
};

export type RollbackPhase =
  | { kind: "idle" }
  | { kind: "waiting"; version: string }
  | { kind: "installing"; version: string }
  | { kind: "error"; message: string };

type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const defaultInvoke: InvokeFn = (command, args) => invoke(command, args);

export type VersionStoreControllerOptions = {
  /** 設定與桌清單都讀成功。到這一步才拉版本庫與桌備份。 */
  initialLoadReady: boolean;
  responding: boolean;
  invokeImpl?: InvokeFn;
};

/**
 * 版本清單、回退預覽、回退、刪版、桌備份。不畫畫面。
 * 回退在 AI 回應中只進 waiting，不呼叫 stopResponse。
 * 一鍵上一版只在清單上有 `previous` 的那一列時才送出。
 */
export function useVersionStoreController({
  initialLoadReady,
  responding,
  invokeImpl = defaultInvoke,
}: VersionStoreControllerOptions) {
  const [versions, setVersions] = useState<VersionList | null>(null);
  const [backups, setBackups] = useState<BackupList | null>(null);
  const [preview, setPreview] = useState<RollbackPreview | null>(null);
  const [phase, setPhase] = useState<RollbackPhase>({ kind: "idle" });
  const [loadError, setLoadError] = useState<string | null>(null);
  const respondingRef = useRef(responding);
  respondingRef.current = responding;
  const versionsRef = useRef(versions);
  versionsRef.current = versions;

  const refresh = useCallback(async () => {
    const [nextVersions, nextBackups] = await Promise.all([
      invokeImpl("list_versions"),
      invokeImpl("list_world_backups"),
    ]);
    setVersions(nextVersions as VersionList);
    setBackups(nextBackups as BackupList);
    setLoadError(null);
  }, [invokeImpl]);

  useEffect(() => {
    if (!initialLoadReady) return;
    let cancelled = false;
    void refresh().catch((reason: unknown) => {
      if (!cancelled) setLoadError(String(reason));
    });
    return () => {
      cancelled = true;
    };
  }, [initialLoadReady, refresh]);

  const runRollback = useCallback(
    async (version: string) => {
      setPhase({ kind: "installing", version });
      try {
        await invokeImpl("rollback_install", { version });
      } catch (reason) {
        setPhase({ kind: "error", message: String(reason) });
      }
    },
    [invokeImpl],
  );

  useEffect(() => {
    if (responding) return;
    if (phase.kind !== "waiting") return;
    void runRollback(phase.version);
  }, [responding, phase, runRollback]);

  const requestRollback = useCallback(
    (version: string) => {
      if (respondingRef.current) {
        setPhase({ kind: "waiting", version });
        return;
      }
      void runRollback(version);
    },
    [runRollback],
  );

  const requestRollbackPrevious = useCallback(() => {
    const row = versionsRef.current?.versions.find((item) => item.previous);
    if (!row) return;
    requestRollback(row.version);
  }, [requestRollback]);

  const loadPreview = useCallback(
    async (version: string) => {
      const next = (await invokeImpl("rollback_preview", { version })) as RollbackPreview;
      setPreview(next);
      return next;
    },
    [invokeImpl],
  );

  const deleteVersion = useCallback(
    async (version: string) => {
      await invokeImpl("delete_version", { version });
      await refresh();
    },
    [invokeImpl, refresh],
  );

  const deleteBackup = useCallback(
    async (worldId: string, kind: BackupKind) => {
      await invokeImpl("delete_world_backup", { worldId, kind });
      await refresh();
    },
    [invokeImpl, refresh],
  );

  return {
    versions,
    backups,
    preview,
    phase,
    loadError,
    refresh,
    loadPreview,
    requestRollback,
    requestRollbackPrevious,
    deleteVersion,
    deleteBackup,
  };
}
