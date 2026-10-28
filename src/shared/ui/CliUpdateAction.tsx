// CLI 錯誤旁與設定頁的更新鈕（plans/cli-version-gate.md 包 2 第 3、6 點）。按下走後端 `update_cli`：
// 先裝已測版（agy 一律最新版），已測版不比目前新或裝了沒用，同一位置改給「裝最新版」；
// 結果只說事實，不自動重送（不替玩家花額度）。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import type { AppConfig } from "../contracts/backend-contracts";
import { backendText } from "./backend-text";
import { CLI_CHANGED_EVENT, OPEN_SETTINGS_EVENT, type CliUpdateOffer } from "./cli-update";

type Outcome =
  | { kind: "updated"; version: string }
  | { kind: "tested_not_newer" }
  | { kind: "already_latest"; version: string }
  | { kind: "failed"; version: string; tested: boolean }
  | { kind: "broken" }
  | { kind: "stuck" }
  | { kind: "busy" }
  | { kind: "already_updating" }
  | { kind: "not_found" };

function openSettingsAi() {
  window.dispatchEvent(new CustomEvent(OPEN_SETTINGS_EVENT, { detail: "ai" }));
}

/** 叫不到的模型是不是玩家自己在設定裡指定的（換 CLI 版本不一定救得了指定的舊模型）。 */
function useCustomModel(offer: CliUpdateOffer): boolean {
  const [custom, setCustom] = useState(false);
  useEffect(() => {
    if (offer.kind !== "model" || !offer.model) return;
    let live = true;
    invoke<AppConfig>("read_config")
      .then((config) => {
        if (!live) return;
        setCustom(
          Object.entries(config.tier_models ?? {}).some(
            ([key, value]) => key.startsWith(`${offer.cli}:`) && value.trim() === offer.model,
          ),
        );
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [offer.cli, offer.kind, offer.model]);
  return custom;
}

export function CliUpdateAction({ offer }: { offer: CliUpdateOffer }) {
  // older＝回覆格式認不得且版本比測過的舊：只裝已測版，不給「裝最新版」
  const testedOnly = offer.kind === "older";
  const [mode, setMode] = useState<"tested" | "latest">("tested");
  const [running, setRunning] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [error, setError] = useState("");
  const customModel = useCustomModel(offer);

  // 換了一則錯誤（另一家 CLI 或另一種原因）：上一則的結果與模式歸零
  useEffect(() => {
    setMode("tested");
    setOutcome(null);
    setError("");
  }, [offer.cli, offer.kind, offer.required]);

  async function run() {
    setRunning(true);
    setError("");
    try {
      const result = await invoke<Outcome>("update_cli", {
        cli: offer.cli,
        mode,
        required: offer.required ?? null,
      });
      setOutcome(result);
      if (result.kind !== "busy" && result.kind !== "tested_not_newer" && result.kind !== "not_found") {
        window.dispatchEvent(new Event(CLI_CHANGED_EVENT));
      }
      // 已測版不比目前新、或已測版裝失敗：同一位置改給「裝最新版」
      const offerLatest =
        !testedOnly &&
        mode === "tested" &&
        (result.kind === "tested_not_newer" || (result.kind === "failed" && result.tested));
      if (offerLatest) setMode("latest");
    } catch (reason) {
      setError(backendText(String(reason)));
    } finally {
      setRunning(false);
    }
  }

  const done = outcome?.kind === "updated" || outcome?.kind === "already_latest";
  const label = running
    ? t("cliUpdRunning")
    : mode === "latest"
      ? t("cliUpdLatest")
      : testedOnly
        ? t("cliUpdTestedOnly")
        : t("cliUpdCheck");
  return (
    <span className="cli-update-action">
      {/* 結果在前、下一步的按鈕在後：讀完結果再決定按不按 */}
      {(outcome || error) && (
        <span className="cli-update-result" role="status">
          {error ? t("cliUpdError", { error }) : outcomeText(outcome!, customModel)}
        </span>
      )}
      {!done && outcome?.kind !== "broken" && outcome?.kind !== "stuck" && (
        <button type="button" className="btn btn-sm" disabled={running} onClick={() => void run()}>
          {label}
        </button>
      )}
      {customModel && (
        <button type="button" className="btn btn-sm" onClick={openSettingsAi}>
          {t("cliUpdChooseModel")}
        </button>
      )}
      {outcome?.kind === "broken" && (
        <button type="button" className="btn btn-sm" onClick={openSettingsAi}>
          {t("cliUpdOpenSettings")}
        </button>
      )}
    </span>
  );
}

function outcomeText(outcome: Outcome, customModel: boolean): string {
  switch (outcome.kind) {
    case "updated":
      return t("cliUpdUpdated", { version: outcome.version });
    case "tested_not_newer":
      return t("cliUpdTestedNotNewer");
    case "already_latest":
      // 有自訂模型就旁邊的「改選模型」是出路；否則要等本 app 更新（不推論成「問題不在 CLI」）
      return customModel ? t("cliUpdAlreadyLatest") : `${t("cliUpdAlreadyLatest")} ${t("cliUpdWaitApp")}`;
    case "failed":
      return t("cliUpdFailed", { version: outcome.version });
    case "broken":
      return t("cliUpdBroken");
    case "stuck":
      return t("cliUpdStuck");
    case "busy":
      return t("cliUpdBusy");
    case "already_updating":
      return t("cliUpdAlready");
    case "not_found":
      return t("cliUpdNotFound");
  }
}
