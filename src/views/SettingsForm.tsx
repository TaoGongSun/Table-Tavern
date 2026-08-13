import { FormEvent, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "../i18n";
import { tierLabel } from "../model-catalog";
import { refreshCatalog, useModelCatalogs } from "../model-catalog-store";
import { AppConfig } from "../backend-contracts";
import { cachedClis, CLI_LABELS, CliInfo, cliConnectedKey, detectClis } from "../cli";

// 檔位預設模型只是設定欄的預填建議（存進 config.json 後由使用者作主），程式邏輯不讀它
const SUGGESTED_TIER_MODELS: Record<string, string> = {
  best: "anthropic/claude-opus-4.8",
  balanced: "anthropic/claude-sonnet-5",
  fast: "google/gemini-3.5-flash",
};

type CliInstallStage = "detect" | "install" | "login" | "verify" | "done" | "error";

interface CliInstallProgress {
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

const CLI_INSTALL_URLS: Record<string, string> = {
  claude: "claude.ai",
  codex: "chatgpt.com/codex",
  agy: "antigravity.google",
  grok: "x.ai/cli",
};

// 系統權限預告只在每家 CLI 第一次啟用時彈一次；說明本身在設定頁常駐，事後查得到
function cliNoticeKey(id: string) {
  return `cli_permission_notice:${id}`;
}

const CLI_RISK_KEYS = ["risk1", "risk2", "risk3", "risk4"] as const;

export function Settings({
  config,
  onSaved,
  onDirty,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onDirty: (count: number) => void;
}) {
  const [apiKey, setApiKey] = useState(config.api_keys["openrouter"] ?? "");
  const [tierModels, setTierModels] = useState<Record<string, string>>({
    ...SUGGESTED_TIER_MODELS,
    ...config.tier_models,
  });
  const [baseUrl, setBaseUrl] = useState(String(config.preferences["base_url"] ?? ""));
  const [imageModel, setImageModel] = useState(String(config.preferences["image_model"] ?? ""));
  const [claudeCompatBaseUrl, setClaudeCompatBaseUrl] = useState(
    String(config.preferences["claude_base_url"] ?? ""),
  );
  const [claudeCompatKey, setClaudeCompatKey] = useState(config.api_keys["claude_compat"] ?? "");
  const [gmTier, setGmTier] = useState(String(config.preferences["gm_tier"] ?? "best"));
  const [maxRound, setMaxRound] = useState(String(config.preferences["max_round_speakers"] ?? 3));
  const [transport, setTransport] = useState(String(config.preferences["transport"] ?? "api"));
  const [permissionNotice, setPermissionNotice] = useState("");
  const [riskAccepted, setRiskAccepted] = useState(config.preferences["cli_risk_accepted"] === true);
  const [clis, setClis] = useState<CliInfo[] | null>(cachedClis());
  const catalogs = useModelCatalogs();
  const [customTiers, setCustomTiers] = useState<Record<string, boolean>>({});
  const [message, setMessage] = useState("");
  const [installingCli, setInstallingCli] = useState<string | null>(null);
  const [installProgress, setInstallProgress] = useState<Record<string, CliInstallProgress>>({});
  const cliPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  function stopCliPolling() {
    if (cliPollRef.current !== null) {
      clearInterval(cliPollRef.current);
      cliPollRef.current = null;
    }
  }

  useEffect(() => {
    detectClis().then(setClis).catch(() => setClis([]));
    // 模型清單不在這裡抓：開 app 就預熱好了（見 prefetchModelCatalogs），
    // 這裡只透過 useModelCatalogs 訂閱結果
    return stopCliPolling;
  }, []);

  // 監聽器只掛一次；config／onSaved 走 ref 取最新值，避免安裝中重掛掉事件
  const configRef = useRef(config);
  configRef.current = config;
  const onSavedRef = useRef(onSaved);
  onSavedRef.current = onSaved;
  useEffect(() => {
    let disposed = false;
    let stopListening: (() => void) | undefined;
    void listen<CliInstallProgress>("cli-install-progress", (event) => {
      setInstallProgress((previous) => ({
        ...previous,
        [event.payload.provider]: event.payload,
      }));
      if (event.payload.stage === "done" || event.payload.stage === "error") {
        setInstallingCli((current) => (current === event.payload.provider ? null : current));
        const base = configRef.current;
        const next = {
          ...base,
          preferences: {
            ...base.preferences,
            [cliConnectedKey(event.payload.provider)]: event.payload.stage === "done",
          },
        };
        void invoke("write_config", { config: next }).then(() => onSavedRef.current(next)).catch(() => {});
        if (event.payload.stage === "done") {
          void detectClis(true).then(setClis).catch(() => {});
          // 剛登入完才拿得到完整清單（未登入時 grok 只回得出一個預設模型），
          // 預熱那次抓的是登入前的結果，這裡補抓一次
          void refreshCatalog(event.payload.provider);
        }
      }
    }).then((unlisten) => {
      if (disposed) {
        unlisten();
      } else {
        stopListening = unlisten;
      }
    });
    return () => {
      disposed = true;
      stopListening?.();
    };
  }, []);

  async function installCli(provider: string) {
    const repeat = installingCli === provider;
    if (!repeat) {
      setInstallingCli(provider);
      setInstallProgress((previous) => {
        const next = { ...previous };
        delete next[provider];
        return next;
      });
    }
    const params = {
      provider: CLI_LABELS[provider],
      url: CLI_INSTALL_URLS[provider],
    };
    try {
      await invoke("install_cli", {
        provider,
        messages: {
          start: t("cliInstallStart", params),
          loginHint: t("cliInstallLoginHint", params),
          success: t("cliInstallSuccess", params),
          fail: t("cliInstallFail", params),
        },
      });
    } catch (reason) {
      const error = String(reason);
      const cooldown = error.match(/^login-cooldown:(\d+)$/);
      setMessage(cooldown ? t("cliLoginCooldown", { secs: cooldown[1] }) : error);
      if (!repeat) setInstallingCli(null);
      return;
    }

    let elapsed = 0;
    stopCliPolling();
    cliPollRef.current = setInterval(() => {
      elapsed += 3_000;
      // 輪詢的目的就是等 CLI 裝好出現，必須繞過快取重探
      void detectClis(true).then(setClis).catch(() => {});
      // 安裝流程跑在獨立終端機／背景工作裡，只能靠 cli_verified 印記知道登入驗證過了沒
      invoke<boolean>("cli_verified", { provider })
        .then((verified) => {
          if (!verified) {
            if (elapsed >= 600_000) {
              stopCliPolling();
              setInstallingCli(null);
            }
            return;
          }
          stopCliPolling();
          setInstallingCli(null);
          const base = configRef.current;
          const next = {
            ...base,
            preferences: { ...base.preferences, [cliConnectedKey(provider)]: true },
          };
          void invoke("write_config", { config: next })
            .then(() => onSavedRef.current(next))
            .catch(() => {});
        })
        .catch(() => {
          if (elapsed >= 600_000) {
            stopCliPolling();
            setInstallingCli(null);
          }
        });
    }, 3_000);
  }

  // 未儲存偵測：與 config 現值逐欄比對（比對值採 save() 相同的正規化），改幾欄算幾項
  const dirtyCount = [
    apiKey.trim() !== (config.api_keys["openrouter"] ?? ""),
    baseUrl.trim() !== String(config.preferences["base_url"] ?? ""),
    imageModel.trim() !== String(config.preferences["image_model"] ?? ""),
    claudeCompatBaseUrl.trim() !== String(config.preferences["claude_base_url"] ?? ""),
    claudeCompatKey.trim() !== (config.api_keys["claude_compat"] ?? ""),
    gmTier !== String(config.preferences["gm_tier"] ?? "best"),
    String(Math.max(1, Number(maxRound) || 3)) !==
      String(config.preferences["max_round_speakers"] ?? 3),
    transport !== String(config.preferences["transport"] ?? "api"),
    riskAccepted !== (config.preferences["cli_risk_accepted"] === true),
    JSON.stringify(tierModels) !==
      JSON.stringify({ ...SUGGESTED_TIER_MODELS, ...config.tier_models }),
  ].filter(Boolean).length;

  useEffect(() => {
    onDirty(dirtyCount);
    return () => onDirty(0);
  }, [dirtyCount, onDirty]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setMessage("");
    if (transport !== "api" && !riskAccepted) {
      setMessage(t("riskRequired"));
      return;
    }
    const next: AppConfig = {
      ...config,
      api_keys: {
        ...config.api_keys,
        openrouter: apiKey.trim(),
        claude_compat: claudeCompatKey.trim(),
      },
      tier_models: tierModels,
      preferences: {
        ...config.preferences,
        base_url: baseUrl.trim(),
        image_model: imageModel.trim(),
        claude_base_url: claudeCompatBaseUrl.trim(),
        transport,
        cli_risk_accepted: riskAccepted,
        gm_tier: gmTier,
        max_round_speakers: Math.max(1, Number(maxRound) || 3),
      },
    };
    try {
      await invoke("write_config", { config: next });
      onSaved(next);
      setMessage(t("saved"));
      if (transport !== "api" && next.preferences[cliNoticeKey(transport)] !== true) {
        setPermissionNotice(transport);
      }
    } catch (reason) {
      setMessage(String(reason));
    }
  }

  // 看過就記下，同一家不再擋；寫失敗就當沒看過（下次再提醒，比默默吞掉好）
  async function ackPermissionNotice() {
    const provider = permissionNotice;
    setPermissionNotice("");
    const next: AppConfig = {
      ...config,
      preferences: { ...config.preferences, [cliNoticeKey(provider)]: true },
    };
    try {
      await invoke("write_config", { config: next });
      onSaved(next);
    } catch {
      /* 記不起來只是下次再問一次，不打斷玩家 */
    }
  }

  return (
    <form id="ai-settings-form" onSubmit={save} className="settings-form">
        <fieldset className="transport-choice">
          <legend>{t("transportLegend")}</legend>
          <label className="inline">
            <input
              type="radio"
              name="transport"
              checked={transport === "api"}
              onChange={() => setTransport("api")}
            />
            {t("transportApi")}
          </label>
          {(["claude", "codex", "agy", "grok"] as const).map((id) => {
            // clis === null＝偵測還沒回來，與「偵測不到」是兩回事：此時不給按鈕，避免誤按一鍵安裝
            const detecting = clis === null;
            const found = clis?.find((c) => c.id === id);
            const progress = installProgress[id];
            const connected = config.preferences[cliConnectedKey(id)] === true;
            return (
              <label key={id} className="inline">
                <input
                  type="radio"
                  name="transport"
                  disabled={!found}
                  checked={transport === id}
                  onChange={() => setTransport(id)}
                />
                {CLI_LABELS[id]}
                {t("cliSubscriptionSuffix")}
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
                          disabled={installingCli !== null && installingCli !== id}
                          onClick={() => void installCli(id)}
                        >
                          {t("cliReverifyBtn")}
                        </button>
                      </>
                    ) : (
                      <button
                        type="button"
                        disabled={installingCli !== null && installingCli !== id}
                        onClick={() => void installCli(id)}
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
                      disabled={installingCli !== null && installingCli !== id}
                      onClick={() => void installCli(id)}
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
                      <span className="cli-install-detail">{progress.detail}</span>
                    )}
                    {progress.logPath && (
                      <small>{t("cliInstallLogPath", { path: progress.logPath })}</small>
                    )}
                  </span>
                )}
              </label>
            );
          })}
        </fieldset>
        {transport !== "api" && (
          <div className="risk-box" role="note">
            <strong>{t("riskTitle")}</strong>
            <ul>
              {CLI_RISK_KEYS.map((key) => (
                <li key={key}>{t(key)}</li>
              ))}
            </ul>
            <label className="inline">
              <input
                type="checkbox"
                checked={riskAccepted}
                onChange={(e) => setRiskAccepted(e.currentTarget.checked)}
              />
              {t("riskAccept")}
            </label>
          </div>
        )}
        {transport !== "api" && (
          <p className="cli-permission-note" role="note">
            {t("cliPermissionNote", { provider: CLI_LABELS[transport] ?? transport })}
          </p>
        )}
        {/* 每家 CLI 第一次啟用時擋一次：此時 CLI 還沒被叫起來，玩家先知道等一下的彈窗是誰在問 */}
        {permissionNotice && (
          <div className="modal-overlay" onClick={() => void ackPermissionNotice()}>
            <div
              className="modal"
              role="dialog"
              aria-modal="true"
              aria-label={t("cliPermissionTitle")}
              onClick={(event) => event.stopPropagation()}
            >
              <h2>{t("cliPermissionTitle")}</h2>
              <p>{t("cliPermissionNote", { provider: CLI_LABELS[permissionNotice] ?? permissionNotice })}</p>
              <div className="ai-gen-footer">
                <button type="button" onClick={() => void ackPermissionNotice()}>
                  {t("cliPermissionAck")}
                </button>
              </div>
            </div>
          </div>
        )}
        {/* OpenRouter 專屬欄位只在 API 直連時顯示，避免 CLI 使用者誤以為必填 */}
        {transport === "api" && (
          <label>
            {t("apiKeyLabel")}
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.currentTarget.value)}
              placeholder="sk-or-..."
            />
          </label>
        )}
        {transport === "api" ? (
          <>
            <label>
              {t("imageModelLabel")}
              <input value={imageModel} onChange={(e) => setImageModel(e.currentTarget.value)} />
            </label>
            {(["best", "balanced", "fast"] as const).map((tier) => (
              <label key={tier}>
                {t("tierModelApiLabel", { tier: tierLabel(tier) })}
                <input
                  list="openrouter-models"
                  value={tierModels[tier] ?? ""}
                  onChange={(e) =>
                    setTierModels({ ...tierModels, [tier]: e.currentTarget.value })
                  }
                />
              </label>
            ))}
            <datalist id="openrouter-models">
              {(catalogs["api"] ?? []).map((m) => (
                <option key={m.id} value={m.id}>
                  {m.label}
                </option>
              ))}
            </datalist>
          </>
        ) : (
          <>
            {(["best", "balanced", "fast"] as const).map((tier) => {
              const key = `${transport}:${tier}`;
              const value = tierModels[key] ?? "";
              const catalog = catalogs[transport] ?? [];
              const custom =
                customTiers[key] ?? (value !== "" && !catalog.some((m) => m.id === value));
              return (
                <label key={key}>
                  {t("tierModelCliLabel", { tier: tierLabel(tier) })}
                  <select
                    value={custom ? "__custom__" : value}
                    onChange={(e) => {
                      const next = e.currentTarget.value;
                      if (next === "__custom__") {
                        setCustomTiers({ ...customTiers, [key]: true });
                      } else {
                        setCustomTiers({ ...customTiers, [key]: false });
                        setTierModels({ ...tierModels, [key]: next });
                      }
                    }}
                  >
                    <option value="">{t("cliDefaultOption")}</option>
                    {catalog.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.label}
                      </option>
                    ))}
                    <option value="__custom__">{t("customModelOption")}</option>
                  </select>
                  {custom && (
                    <input
                      value={value}
                      placeholder={t("customModelPlaceholder")}
                      onChange={(e) =>
                        setTierModels({ ...tierModels, [key]: e.currentTarget.value })
                      }
                    />
                  )}
                </label>
              );
            })}
            <p className="cli-version" role="note">
              {transport === "claude"
                ? t("cliCatalogClaude")
                : transport === "agy"
                  ? t("cliCatalogAgy")
                  : transport === "grok"
                    ? t("cliCatalogGrok")
                  : t("cliCatalogCodex")}
            </p>
          </>
        )}
        {transport === "claude" && (
          <details>
            <summary>{t("claudeCompatSummary")}</summary>
            <label>
              {t("claudeCompatBaseUrlLabel")}
              <input
                value={claudeCompatBaseUrl}
                onChange={(e) => setClaudeCompatBaseUrl(e.currentTarget.value)}
                placeholder="https://api.example.com"
              />
            </label>
            <label>
              {t("claudeCompatKeyLabel")}
              <input
                type="password"
                value={claudeCompatKey}
                onChange={(e) => setClaudeCompatKey(e.currentTarget.value)}
              />
            </label>
            <p role="note">{t("claudeCompatHint")}</p>
          </details>
        )}
        <label>
          {t("gmTierLabel")}
          <select value={gmTier} onChange={(e) => setGmTier(e.currentTarget.value)}>
            {(["best", "balanced", "fast"] as const).map((tier) => (
              <option key={tier} value={tier}>
                {tierLabel(tier)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t("maxRoundLabel")}
          <input
            type="number"
            min={1}
            max={10}
            value={maxRound}
            onChange={(e) => setMaxRound(e.currentTarget.value)}
          />
        </label>
        {transport === "api" && (
          <label>
            {t("baseUrlLabel")}
            <input
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.currentTarget.value)}
              placeholder="https://openrouter.ai/api/v1"
            />
          </label>
        )}
      {message && (
        <div className="row">
          <span role="status">{message}</span>
        </div>
      )}
    </form>
  );
}
