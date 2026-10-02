import { useCallback, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { confirm, message } from "@tauri-apps/plugin-dialog";
import { t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";
import { useUpdateController, type UpdateControllerOptions } from "./useUpdateController";
import { useVersionStoreController } from "./useVersionStoreController";
import {
  rollbackDialogText,
  rollbackFlowBusy,
  rollbackNotice,
  updateFlowBusy,
  type RollbackCheck,
} from "./version-center";

type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export type VersionCenterOptions = {
  configLoaded: boolean;
  /** 設定與桌清單都讀成功。 */
  initialLoadReady: boolean;
  preferences: Record<string, unknown> | undefined;
  responding: boolean;
  stopResponse: () => void;
  onConfig: (config: AppConfig) => void;
  invokeImpl?: InvokeFn;
  listenImpl?: UpdateControllerOptions["listenImpl"];
  askNoRollback?: () => Promise<boolean>;
  askRollback?: (version: string, check: RollbackCheck, currentVersion: string) => Promise<boolean>;
  readVersion?: () => Promise<string>;
};

/** 目標不合格只給「關閉」；其餘是「繼續／取消」。 */
async function askRollbackNative(
  version: string,
  check: RollbackCheck,
  currentVersion: string,
): Promise<boolean> {
  const notice = rollbackNotice(check);
  const body = rollbackDialogText(notice, currentVersion);
  const title = t("rollbackTitle", { version });
  if (notice.kind === "invalid") {
    await message(body, { title, kind: "error", okLabel: t("closeBtn") });
    return false;
  }
  return confirm(body, {
    title,
    kind: "warning",
    okLabel: t("updateContinue"),
    cancelLabel: t("updateCancel"),
  });
}

/**
 * 更新與回退接在一起：啟動後整理完成才讀版本清單、下載完成刷新清單、
 * 兩個流程共用一個忙碌狀態（任一個在跑，另一個不能開始）。等待狀態在這裡，關掉設定頁不會取消。
 */
export function useVersionCenter({
  configLoaded,
  initialLoadReady,
  preferences,
  responding,
  stopResponse,
  onConfig,
  invokeImpl,
  listenImpl,
  askNoRollback,
  askRollback = askRollbackNative,
  readVersion = getVersion,
}: VersionCenterOptions) {
  const [appVersion, setAppVersion] = useState<string | null>(null);
  useEffect(() => {
    void readVersion()
      .then(setAppVersion)
      .catch(() => {});
  }, [readVersion]);

  const storeBusy = useRef<() => boolean>(() => false);
  const refreshList = useRef<() => void>(() => {});
  const update = useUpdateController({
    configLoaded,
    initialLoadReady,
    preferences,
    responding,
    onConfig,
    isBlocked: () => storeBusy.current(),
    onDownloaded: () => refreshList.current(),
    askNoRollback,
    invokeImpl,
    listenImpl,
  });
  const currentVersion = appVersion ?? update.offer?.current_version ?? "";
  const ask = useCallback(
    (version: string, check: RollbackCheck) => askRollback(version, check, currentVersion),
    [askRollback, currentVersion],
  );
  const store = useVersionStoreController({
    initialLoadReady: update.launchSettled,
    responding,
    isBlocked: update.isFlowBusy,
    askRollback: ask,
    onConfig,
    invokeImpl,
  });
  storeBusy.current = store.isFlowBusy;
  refreshList.current = () => void store.refresh();

  return {
    appVersion,
    update,
    store,
    busy: updateFlowBusy(update.phase) || rollbackFlowBusy(store.phase),
    stopResponse,
  };
}

export type VersionCenter = ReturnType<typeof useVersionCenter>;
