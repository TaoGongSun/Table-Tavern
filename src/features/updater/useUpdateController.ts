import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { confirm } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";
import { updateConfig } from "../settings/update-config";
import type { AppConfig } from "../../shared/contracts/backend-contracts";
import {
  startupReminder,
  updateChanged,
  updateFlowBusy,
  type StartupReminder,
} from "./version-center";

export type UpdateLevel = "format" | "patch" | "feature";

export type UpdateOffer = {
  version: string;
  current_version: string;
  notes: string | null;
  pub_date: string | null;
  level: UpdateLevel;
  skipped: boolean;
};

/** 對應後端 CheckResult（serde tag = status）。 */
export type CheckResult =
  | { status: "available"; offer: UpdateOffer }
  | { status: "none" }
  | { status: "failed"; message: string };

export type UpdatePhase =
  | { kind: "idle" }
  | { kind: "available"; offer: UpdateOffer }
  | { kind: "waiting"; offer: UpdateOffer }
  | { kind: "downloading"; offer: UpdateOffer; downloaded: number; total: number | null }
  | { kind: "confirm"; offer: UpdateOffer }
  /** 要更新的版本被換掉，正在重新檢查。算流程進行中。 */
  | { kind: "rechecking"; offer: UpdateOffer }
  | { kind: "installing"; offer: UpdateOffer }
  | { kind: "error"; offer: UpdateOffer; message: string };

const DAY_MS = 24 * 60 * 60 * 1000;

type ProgressPayload = { downloaded: number; total: number | null };

type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
type ListenFn = (
  event: string,
  handler: (event: { payload: ProgressPayload }) => void,
) => Promise<UnlistenFn>;

// 預設實作放模組層。參數預設的箭頭函式每次 render 都是新的，自動檢查會跟著重跑。
const defaultInvoke: InvokeFn = (command, args) => invoke(command, args);
const defaultListen: ListenFn = (event, handler) => listen<ProgressPayload>(event, handler);
const defaultSave = (patch: Record<string, unknown>) => updateConfig(patch);
const defaultAskNoRollback = () =>
  confirm(t("updateNoRollbackBody"), {
    title: t("updateNoRollbackTitle"),
    kind: "warning",
    okLabel: t("updateContinue"),
    cancelLabel: t("dialogCancel"),
  });
const notBlocked = () => false;
const noop = () => {};

export type DownloadResult = {
  version: string;
  rollback_ready: boolean;
};

export type UpdateControllerOptions = {
  configLoaded: boolean;
  /** 設定與桌清單都讀成功之後才呼叫 update_post_launch。 */
  initialLoadReady: boolean;
  preferences: Record<string, unknown> | undefined;
  responding: boolean;
  onConfig: (config: AppConfig) => void;
  /** 回退流程進行中：不能開始更新，檢查結果也不改更新資訊。 */
  isBlocked?: () => boolean;
  /** 下載完成（接著不論安裝、取消或失敗）時呼叫，用來刷新版本清單。 */
  onDownloaded?: () => void;
  /** 沒有回退點時問玩家要不要繼續。 */
  askNoRollback?: () => Promise<boolean>;
  invokeImpl?: InvokeFn;
  listenImpl?: ListenFn;
  intervalMs?: number;
  savePreference?: (patch: Record<string, unknown>) => Promise<AppConfig>;
};

function offerOf(phase: UpdatePhase): UpdateOffer | null {
  return phase.kind === "idle" ? null : phase.offer;
}

/**
 * 啟動檢查一次，之後每 24 小時一次。`update_auto_check` 不是 false 才自動檢查
 * （缺鍵、非 bool 都當開）。這個開關中途打開會立刻再查一次。
 * `update_post_launch` 等設定與桌清單都讀成功（`initialLoadReady`）才呼叫一次，
 * 完成（成功或失敗）後 `launchSettled` 才轉真。
 * 檢查結果：`available` 換成新的更新資訊、`none` 清掉、`failed` 保留原本的。
 * 流程進行中（等待、下載、確認、安裝，或回退在跑）任何檢查結果都不改狀態。
 * 啟動提醒只看啟動那次自動檢查。AI 回應中按更新只進 waiting，不自己呼叫 stopResponse。
 */
export function useUpdateController({
  configLoaded,
  initialLoadReady,
  preferences,
  responding,
  onConfig,
  isBlocked = notBlocked,
  onDownloaded = noop,
  askNoRollback = defaultAskNoRollback,
  invokeImpl = defaultInvoke,
  listenImpl = defaultListen,
  intervalMs = DAY_MS,
  savePreference = defaultSave,
}: UpdateControllerOptions) {
  const [phase, setPhaseState] = useState<UpdatePhase>({ kind: "idle" });
  const [launchSettled, setLaunchSettled] = useState(false);
  const [checking, setChecking] = useState(false);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [checked, setChecked] = useState(false);
  const [reminder, setReminder] = useState<{ kind: StartupReminder; offer: UpdateOffer } | null>(
    null,
  );
  // phaseRef 同步跟著寫：同一個事件裡的第二次按鈕與回退那邊的互斥都要看到最新值。
  const phaseRef = useRef(phase);
  const setPhase = useCallback((next: UpdatePhase) => {
    phaseRef.current = next;
    setPhaseState(next);
  }, []);
  const respondingRef = useRef(responding);
  respondingRef.current = responding;
  const preferencesRef = useRef(preferences);
  preferencesRef.current = preferences;
  const blockedRef = useRef(isBlocked);
  blockedRef.current = isBlocked;
  const downloadedRef = useRef(onDownloaded);
  downloadedRef.current = onDownloaded;
  const askRef = useRef(askNoRollback);
  askRef.current = askNoRollback;
  const posted = useRef(false);
  const startupHandled = useRef(false);
  const reminded = useRef(new Set<string>());

  useEffect(() => {
    if (!initialLoadReady || posted.current) return;
    posted.current = true;
    void invokeImpl("update_post_launch")
      .catch(() => {})
      .finally(() => setLaunchSettled(true));
  }, [initialLoadReady, invokeImpl]);

  const flowBusy = useCallback(
    () => updateFlowBusy(phaseRef.current) || blockedRef.current(),
    [],
  );

  /**
   * `failed` 由呼叫端處理；這裡只套 available／none。
   * 啟動提醒只留給它指向的那一版：none 或換成別版都收掉（新版不在啟動那次檢查，不另出提醒）。
   */
  const apply = useCallback(
    (result: Exclude<CheckResult, { status: "failed" }>) => {
      setChecked(true);
      const next = result.status === "available" ? result.offer : null;
      setReminder((current) => (current && current.offer.version === next?.version ? current : null));
      setPhase(next ? { kind: "available", offer: next } : { kind: "idle" });
    },
    [setPhase],
  );

  const adopt = useCallback(
    (result: CheckResult) => {
      if (result.status === "failed" || flowBusy()) return;
      apply(result);
    },
    [flowBusy, apply],
  );

  const auto = preferences?.update_auto_check !== false;
  useEffect(() => {
    if (!configLoaded) return;
    // 啟動時自動檢查是關的：之後再打開也不算啟動那一次。
    if (!auto) {
      startupHandled.current = true;
      return;
    }
    let cancelled = false;
    let first = true;
    const run = () => {
      // 流程進行中結果本來就不採用，不必連網。
      if (!first && flowBusy()) return;
      const startup = first;
      first = false;
      void invokeImpl("update_check", { manual: false })
        .then((value) => {
          if (cancelled) return;
          const result = value as CheckResult;
          adopt(result);
          if (!startup || startupHandled.current) return;
          startupHandled.current = true;
          if (result.status !== "available") return;
          const kind = startupReminder(result.offer, preferencesRef.current);
          if (kind) setReminder({ kind, offer: result.offer });
        })
        .catch(() => {});
    };
    run();
    const timer = setInterval(run, intervalMs);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [configLoaded, auto, invokeImpl, intervalMs, adopt, flowBusy]);

  const reloadConfig = useCallback(async () => {
    try {
      onConfig((await invokeImpl("read_config")) as AppConfig);
    } catch {
      // 讀不回設定仍顯示安裝錯誤。
    }
  }, [invokeImpl, onConfig]);

  const install = useCallback(
    async (offer: UpdateOffer, version: string) => {
      setPhase({ kind: "installing", offer });
      try {
        await invokeImpl("update_install", { version });
      } catch (reason) {
        // 略過鍵在開閘前就清了。安裝失敗時閘已放下，重讀設定，前端才不會停在舊值。
        await reloadConfig();
        setPhase({ kind: "error", offer, message: String(reason) });
      }
    },
    [invokeImpl, reloadConfig, setPhase],
  );

  /** 要更新的版本被換掉：不安裝，重新檢查並顯示最新的更新資訊，讓玩家再按一次。 */
  // 重新檢查期間維持忙碌（更新鈕停用、回退被擋）；結果只在 phase 仍是這一次時才套用，過期的一律丟棄。
  const showLatest = useCallback(
    async (offer: UpdateOffer) => {
      const marker: UpdatePhase = { kind: "rechecking", offer };
      setPhase(marker);
      let result: CheckResult;
      try {
        result = (await invokeImpl("update_check", { manual: true })) as CheckResult;
      } catch (reason) {
        result = { status: "failed", message: String(reason) };
      }
      if (phaseRef.current !== marker) return;
      if (result.status === "failed") setPhase({ kind: "error", offer, message: result.message });
      else apply(result);
    },
    [invokeImpl, apply, setPhase],
  );

  const runUpdate = useCallback(
    async (offer: UpdateOffer) => {
      setPhase({ kind: "downloading", offer, downloaded: 0, total: null });
      let unlisten: UnlistenFn = () => {};
      let downloaded: DownloadResult;
      try {
        unlisten = await listenImpl("update-progress", (event) => {
          const current = phaseRef.current;
          if (current.kind !== "downloading") return;
          setPhase({ ...current, downloaded: event.payload.downloaded, total: event.payload.total });
        });
        // 帶玩家看到的那一版：後端槽已被後來的檢查換掉就不下載。
        const result = (await invokeImpl("update_download", {
          version: offer.version,
        })) as DownloadResult | null;
        if (!result || typeof result.version !== "string" || result.version.length === 0) {
          // 借後端同一碼（UpdateNotDownloaded），顯示時跟其他錯誤一樣走 backendText。
          throw 'TTMSG:{"code":"update_not_downloaded"}';
        }
        downloaded = result;
      } catch (reason) {
        if (updateChanged(String(reason))) {
          await showLatest(offer);
          return;
        }
        setPhase({ kind: "error", offer, message: String(reason) });
        return;
      } finally {
        unlisten();
      }
      downloadedRef.current();
      // 每次按更新都看這次下載的 rollback_ready，不沿用上次。
      if (downloaded.rollback_ready !== true) {
        setPhase({ kind: "confirm", offer });
        let proceed = false;
        try {
          proceed = await askRef.current();
        } catch {
          proceed = false;
        }
        if (!proceed) {
          setPhase({ kind: "available", offer });
          return;
        }
      }
      await install(offer, downloaded.version);
    },
    [invokeImpl, listenImpl, install, setPhase, showLatest],
  );

  useEffect(() => {
    if (responding) return;
    if (phase.kind !== "waiting") return;
    void runUpdate(phase.offer);
  }, [responding, phase, runUpdate]);

  const requestInstall = useCallback(() => {
    const current = phaseRef.current;
    if (current.kind !== "available" && current.kind !== "error") return;
    if (blockedRef.current()) return;
    if (respondingRef.current) {
      setPhase({ kind: "waiting", offer: current.offer });
      return;
    }
    void runUpdate(current.offer);
  }, [runUpdate, setPhase]);

  const cancelWait = useCallback(() => {
    const current = phaseRef.current;
    if (current.kind === "waiting") setPhase({ kind: "available", offer: current.offer });
  }, [setPhase]);

  const checkNow = useCallback(async () => {
    setChecking(true);
    setCheckError(null);
    try {
      const result = (await invokeImpl("update_check", { manual: true })) as CheckResult;
      if (result.status === "failed") setCheckError(result.message);
      else adopt(result);
    } catch (reason) {
      setCheckError(String(reason));
    } finally {
      setChecking(false);
    }
  }, [invokeImpl, adopt]);

  const saveSkipped = useCallback(
    async (version: string | null) => {
      onConfig(await savePreference({ preferences: { update_skipped_version: version } }));
    },
    [onConfig, savePreference],
  );

  /** 略過後仍留在更新區（標「已略過」），小點熄掉。 */
  const skip = useCallback((version: string) => saveSkipped(version), [saveSkipped]);
  const unskip = useCallback(() => saveSkipped(null), [saveSkipped]);

  /** 橫幅或對話框真的畫出來之後才呼叫。同一版本次執行只寫一次。 */
  const markReminderShown = useCallback(
    (version: string) => {
      if (reminded.current.has(version)) return;
      reminded.current.add(version);
      void savePreference({ preferences: { update_reminded_version: version } })
        .then(onConfig)
        .catch(() => {});
    },
    [onConfig, savePreference],
  );

  const dismissReminder = useCallback(() => setReminder(null), []);

  return {
    phase,
    offer: offerOf(phase),
    launchSettled,
    checking,
    checked,
    checkError,
    reminder,
    isFlowBusy: () => updateFlowBusy(phaseRef.current),
    skip,
    unskip,
    requestInstall,
    cancelWait,
    checkNow,
    markReminderShown,
    dismissReminder,
  };
}

export type UpdateController = ReturnType<typeof useUpdateController>;
