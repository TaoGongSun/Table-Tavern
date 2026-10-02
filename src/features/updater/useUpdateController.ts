import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { updateConfig } from "../settings/update-config";
import type { AppConfig } from "../../shared/contracts/backend-contracts";

export type UpdateLevel = "format" | "patch" | "feature";

export type UpdateOffer = {
  version: string;
  current_version: string;
  notes: string | null;
  pub_date: string | null;
  level: UpdateLevel;
  skipped: boolean;
};

export type UpdatePhase =
  | { kind: "idle" }
  | { kind: "available"; offer: UpdateOffer }
  | { kind: "waiting"; offer: UpdateOffer }
  | { kind: "downloading"; offer: UpdateOffer; downloaded: number; total: number | null }
  | { kind: "installing"; offer: UpdateOffer }
  | { kind: "error"; message: string };

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

function keepInflight(current: UpdatePhase, offer: UpdateOffer | null): UpdatePhase {
  if (
    current.kind === "waiting" ||
    current.kind === "downloading" ||
    current.kind === "installing"
  ) {
    return current;
  }
  return offer ? { kind: "available", offer } : { kind: "idle" };
}

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
  stopResponse: () => void;
  onConfig: (config: AppConfig) => void;
  invokeImpl?: InvokeFn;
  listenImpl?: ListenFn;
  intervalMs?: number;
  savePreference?: (patch: Record<string, unknown>) => Promise<AppConfig>;
};

function offerOf(phase: UpdatePhase): UpdateOffer | null {
  if (phase.kind === "available" || phase.kind === "waiting" || phase.kind === "downloading" || phase.kind === "installing") {
    return phase.offer;
  }
  return null;
}

/**
 * 啟動檢查一次，之後每 24 小時一次。`update_auto_check` 不是 false 才自動檢查
 * （缺鍵、非 bool 都當開）。這個開關中途打開會立刻再查一次。
 * `update_post_launch` 等設定與桌清單都讀成功（`initialLoadReady`）才呼叫一次。
 * 不畫任何提示。回傳的 phase／rollbackReady／skip／requestInstall／stopResponse 留給包 5。
 * AI 回應中按安裝只進 waiting，不自己呼叫 stopResponse。
 */
export function useUpdateController({
  configLoaded,
  initialLoadReady,
  preferences,
  responding,
  stopResponse,
  onConfig,
  invokeImpl = defaultInvoke,
  listenImpl = defaultListen,
  intervalMs = DAY_MS,
  savePreference = defaultSave,
}: UpdateControllerOptions) {
  const [phase, setPhase] = useState<UpdatePhase>({ kind: "idle" });
  const [rollbackReady, setRollbackReady] = useState<boolean | null>(null);
  const phaseRef = useRef(phase);
  phaseRef.current = phase;
  const respondingRef = useRef(responding);
  respondingRef.current = responding;
  const inflight = useRef(false);
  const posted = useRef(false);

  useEffect(() => {
    if (!initialLoadReady || posted.current) return;
    posted.current = true;
    void invokeImpl("update_post_launch").catch(() => {});
  }, [initialLoadReady, invokeImpl]);

  const auto = preferences?.update_auto_check !== false;
  useEffect(() => {
    if (!configLoaded || !auto) return;
    let cancelled = false;
    const run = () => {
      void invokeImpl("update_check", { manual: false })
        .then((value) => {
          if (cancelled) return;
          const offer = value as UpdateOffer | null;
          setPhase((current) => keepInflight(current, offer));
        })
        .catch(() => {});
    };
    run();
    const timer = setInterval(run, intervalMs);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [configLoaded, auto, invokeImpl, intervalMs]);

  const runInstall = useCallback(
    async (offer: UpdateOffer) => {
      if (inflight.current) return;
      inflight.current = true;
      setPhase({ kind: "downloading", offer, downloaded: 0, total: null });
      let unlisten: UnlistenFn = () => {};
      try {
        unlisten = await listenImpl("update-progress", (event) => {
          setPhase((current) =>
            current.kind === "downloading"
              ? { ...current, downloaded: event.payload.downloaded, total: event.payload.total }
              : current,
          );
        });
        const downloaded = (await invokeImpl("update_download")) as DownloadResult | null;
        if (!downloaded || typeof downloaded.version !== "string" || downloaded.version.length === 0) {
          throw "尚未下載";
        }
        setRollbackReady(downloaded.rollback_ready === true);
        setPhase({ kind: "installing", offer });
        try {
          await invokeImpl("update_install", { version: downloaded.version });
        } catch (reason) {
          // 略過鍵在開閘前就清了。安裝失敗時閘已放下，重讀設定，前端才不會停在舊值。
          try {
            onConfig((await invokeImpl("read_config")) as AppConfig);
          } catch {
            // 讀不回設定仍顯示安裝錯誤。
          }
          throw reason;
        }
      } catch (reason) {
        setPhase({ kind: "error", message: String(reason) });
      } finally {
        inflight.current = false;
        unlisten();
      }
    },
    [invokeImpl, listenImpl, onConfig],
  );

  useEffect(() => {
    if (responding) return;
    if (phase.kind !== "waiting") return;
    void runInstall(phase.offer);
  }, [responding, phase, runInstall]);

  const requestInstall = useCallback(() => {
    const offer = offerOf(phaseRef.current);
    if (!offer) return;
    if (respondingRef.current) {
      setPhase({ kind: "waiting", offer });
      return;
    }
    void runInstall(offer);
  }, [runInstall]);

  const checkNow = useCallback(async () => {
    try {
      const offer = (await invokeImpl("update_check", { manual: true })) as UpdateOffer | null;
      setPhase((current) => keepInflight(current, offer));
    } catch (reason) {
      setPhase({ kind: "error", message: String(reason) });
    }
  }, [invokeImpl]);

  const skip = useCallback(
    async (version: string) => {
      const saved = await savePreference({ preferences: { update_skipped_version: version } });
      onConfig(saved);
      setPhase({ kind: "idle" });
    },
    [onConfig, savePreference],
  );

  return { phase, rollbackReady, skip, requestInstall, checkNow, stopResponse };
}
