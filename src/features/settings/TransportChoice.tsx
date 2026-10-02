// AI 分頁的連線方式清單：每列一個有框的選項（radio＋名稱是 label），
// 偵測狀態、驗證／換帳號／安裝鈕與安裝進度是同列的兄弟節點，不包進 label 裡誤觸切換。
import { t } from "../../i18n";
import { backendText } from "../../shared/ui/backend-text";
import { CLI_LABELS, CliInfo, cliConnectedKey } from "../ai-connection/cli";
import { AppConfig } from "../../shared/contracts/backend-contracts";

type CliInstallStage = "detect" | "install" | "login" | "verify" | "done" | "error";

export interface CliInstallProgress {
  provider: string;
  stage: CliInstallStage;
  detail?: string;
  logPath?: string;
}

function cliInstallStageText(stage: CliInstallStage) {
  switch (stage) {
    case "detect":
      return t("cliInstallStageDetect");
    case "install":
      return t("cliInstallStageInstall");
    case "login":
      return t("cliInstallStageLogin");
    case "verify":
      return t("cliInstallStageVerify");
    case "done":
      return t("cliInstallStageDone");
    case "error":
      return t("cliInstallStageError");
  }
}

// PowerShell 的錯誤代號與安裝腳本自身的訊息固定為英文，不受系統語言影響，
// 可靠地認出「安裝檔被其他程序鎖住」（防毒掃描中、工具還在跑）這類失敗
const FILE_LOCKED_MARKERS = [
  "RemoveFileSystemItemIOError",
  "being used by another process",
  "Failed to install",
];

// 連不上服務商（下載失敗的 PowerShell 錯誤代號）或登入視窗沒走完（我們自己的錯誤字串）
const NETWORK_MARKERS = [
  "InvokeRestMethodCommand",
  "InvokeWebRequestCommand",
  "login window closed or timed out",
  "verification failed",
];

function cliInstallErrorHint(detail: string | undefined) {
  if (!detail) {
    return null;
  }
  if (FILE_LOCKED_MARKERS.some((marker) => detail.includes(marker))) {
    return t("cliInstallHintFileLocked");
  }
  if (NETWORK_MARKERS.some((marker) => detail.includes(marker))) {
    return t("cliInstallHintNetwork");
  }
  return null;
}

// 有非互動登出指令的才換得了帳號；agy 只有 TUI 裡的 /logout，那列維持「重新驗證」
const CLI_SWITCHABLE = new Set(["claude", "codex", "grok"]);

const optionClass = (on: boolean) => (on ? "transport-option transport-option-on" : "transport-option");

export function TransportChoice({
  transport,
  onTransport,
  clis,
  config,
  installingCli,
  installProgress,
  onInstall,
}: {
  transport: string;
  onTransport: (id: string) => void;
  /** null＝偵測還沒回來 */
  clis: CliInfo[] | null;
  config: AppConfig;
  installingCli: string | null;
  installProgress: Record<string, CliInstallProgress>;
  onInstall: (id: string, switchAccount: boolean) => void;
}) {
  return (
    <fieldset className="transport-list">
      <legend>{t("transportLegend")}</legend>
      <div className={optionClass(transport === "api")}>
        <label className="transport-option-name">
          <input
            type="radio"
            name="transport"
            checked={transport === "api"}
            onChange={() => onTransport("api")}
          />
          {t("transportApi")}
        </label>
      </div>
      {(["claude", "codex", "agy", "grok"] as const).map((id) => {
        // clis === null＝偵測還沒回來，與「偵測不到」是兩回事：此時不給按鈕，避免誤按一鍵安裝
        const detecting = clis === null;
        const found = clis?.find((c) => c.id === id);
        const progress = installProgress[id];
        const connected = config.preferences[cliConnectedKey(id)] === true;
        const othersBusy = installingCli !== null && installingCli !== id;
        return (
          <div
            key={id}
            className={optionClass(transport === id)}
          >
            <label className="transport-option-name">
              <input
                type="radio"
                name="transport"
                disabled={!found}
                checked={transport === id}
                onChange={() => onTransport(id)}
              />
              {CLI_LABELS[id]}
              {t("cliSubscriptionSuffix")}
            </label>
            {detecting ? (
              <span className="cli-version" role="status">
                {t("cliDetecting")}
              </span>
            ) : found ? (
              <>
                <span className="cli-version">{t("cliDetected", { version: found.version })}</span>
                {connected && installingCli !== id ? (
                  <>
                    <span className="cli-connected">{t("cliConnectedBadge")}</span>
                    <button
                      type="button"
                      className="btn btn-sm"
                      disabled={othersBusy}
                      onClick={() => onInstall(id, false)}
                    >
                      {t("cliReverifyBtn")}
                    </button>
                    {/* 換帳號要先登出，取消登入就回不去舊帳號，所以跟重新驗證分成兩顆 */}
                    {CLI_SWITCHABLE.has(id) && (
                      <button
                        type="button"
                        className="btn btn-sm"
                        disabled={othersBusy}
                        onClick={() => onInstall(id, true)}
                      >
                        {t("cliSwitchAccountBtn")}
                      </button>
                    )}
                  </>
                ) : (
                  <button
                    type="button"
                    className="btn btn-sm"
                    disabled={othersBusy}
                    onClick={() => onInstall(id, false)}
                  >
                    {installingCli === id
                      ? t("cliInstalling", { provider: CLI_LABELS[id] })
                      : t("cliLoginVerifyBtn")}
                  </button>
                )}
              </>
            ) : (
              <>
                <span className="cli-version">{t("cliNotDetected")}</span>
                <button
                  type="button"
                  className="btn btn-sm"
                  disabled={othersBusy}
                  onClick={() => onInstall(id, false)}
                >
                  {installingCli === id
                    ? t("cliInstalling", { provider: CLI_LABELS[id] })
                    : t("cliInstallBtn")}
                </button>
              </>
            )}
            {progress && (
              <span
                className={`cli-install-progress${progress.stage === "error" ? " cli-install-error" : ""}`}
                role={progress.stage === "error" ? "alert" : "status"}
              >
                <strong>{cliInstallStageText(progress.stage)}</strong>
                {progress.stage === "error" && cliInstallErrorHint(progress.detail) && (
                  <span className="cli-install-hint">{cliInstallErrorHint(progress.detail)}</span>
                )}
                {progress.detail && (
                  <span className="cli-install-detail">{backendText(progress.detail)}</span>
                )}
                {progress.logPath && (
                  <small>{t("cliInstallLogPath", { path: progress.logPath })}</small>
                )}
              </span>
            )}
          </div>
        );
      })}
    </fieldset>
  );
}
