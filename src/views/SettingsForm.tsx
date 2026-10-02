import { FormEvent, type ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "../i18n";
import { checkApiKey } from "../features/ai-connection/api-key-check";
import { tierLabel } from "../features/ai-connection/model-catalog";
import { refreshCatalog, useModelCatalogs } from "../features/ai-connection/model-catalog-store";
import { updateConfig } from "../features/settings/update-config";
import { AppConfig } from "../shared/contracts/backend-contracts";
import { cachedClis, CLI_LABELS, CliInfo, cliConnectedKey, detectClis } from "../features/ai-connection/cli";
import { CACHE_UPDATED_EVENT } from "./SmartFreeNewModelBanner";
import { type CliInstallProgress, TransportChoice } from "./TransportChoice";

// 檔位預設模型只是設定欄的預填建議（存進 config.json 後由使用者作主），程式邏輯不讀它
const SUGGESTED_TIER_MODELS: Record<string, string> = {
  best: "anthropic/claude-opus-4.8",
  balanced: "anthropic/claude-sonnet-5",
  fast: "google/gemini-3.5-flash",
};

interface SmartFreeRecommendation {
  model: string;
  label: string;
  expiresAt: number | null;
  showExpiryDate: boolean;
  expiringSoon: boolean;
  reason: string;
  provider: string;
  recent: boolean;
  contextLength: number;
  longContext: boolean;
}

interface SmartFreeRecommendationList {
  limited: SmartFreeRecommendation[];
  // 前兩名：[0] 是自動模式送出的那支，[1] 供玩家在第一名失效時手動改用。
  stable: SmartFreeRecommendation[];
}

const EMPTY_RECOMMENDATIONS: SmartFreeRecommendationList = { limited: [], stable: [] };

function normalizeApiModelMode(value: unknown) {
  const mode = String(value ?? "manual");
  return mode === "smart_free" ? "stable_free" : mode;
}

function formatRecommendationDate(timestamp: number) {
  return new Date(timestamp * 1000).toISOString().slice(0, 10);
}

function formatContext(tokens: number) {
  if (tokens >= 1_000_000) return `${Math.round(tokens / 100_000) / 10}M`;
  return `${Math.round(tokens / 1000)}K`;
}

function recommendationReason(model: SmartFreeRecommendation) {
  let primary: string;
  switch (model.reason) {
    case "anonymous_test":
      primary = t("smartFreeReasonAnonymousTest");
      break;
    case "provider_test":
      primary = t("smartFreeReasonProviderTest", { provider: model.provider });
      break;
    case "limited_test":
      primary = t("smartFreeReasonLimitedTest");
      break;
    case "provider_limited":
      primary = t("smartFreeReasonProviderLimited", { provider: model.provider });
      break;
    case "stable_roleplay":
      return t("smartFreeReasonStableRoleplay");
    case "stable_weekly":
      return t("smartFreeReasonStableWeekly");
    case "stable_available":
      return t("smartFreeReasonStableAvailable");
    default:
      primary = t("smartFreeReasonLimited");
      break;
  }
  const details = [primary];
  if (model.recent) details.push(t("smartFreeReasonNew"));
  if (model.longContext) {
    details.push(t("smartFreeReasonLongContext", { context: formatContext(model.contextLength) }));
  }
  return details.join(" · ");
}

function recommendationAvailability(model: SmartFreeRecommendation) {
  const details: string[] = [];
  if (model.expiringSoon) details.push(t("smartFreeExpiringSoon"));
  if (model.expiresAt && model.showExpiryDate) {
    details.push(t("smartFreeUntil", { date: formatRecommendationDate(model.expiresAt) }));
  } else if (
    model.reason !== "stable_roleplay" &&
    model.reason !== "stable_weekly" &&
    model.reason !== "stable_available"
  ) {
    details.push(t("smartFreeExperimentalAvailability"));
  }
  return details.join(" · ");
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

type SaveMessage = { kind: "ok" | "error"; text: string };

// AI 分頁：表單填滿分頁＝欄位捲動區＋固定在底部的儲存列（未儲存提示、返回／不儲存返回、儲存設定）。
// 儲存中或 CLI 權限提示開著時要擋住設定視窗的所有離開路徑，所以兩者一變就同步回報 onBlockingChange。
export function Settings({
  config,
  onSaved,
  onDirty,
  onBack,
  onBlockingChange,
  children,
}: {
  config: AppConfig;
  onSaved: (c: AppConfig) => void;
  onDirty: (count: number) => void;
  /** 儲存列左鈕：乾淨時「返回」、有修改時「不儲存返回」，同一個動作 */
  onBack: () => void;
  onBlockingChange: (blocked: boolean) => void;
  /** 接在捲動區尾端、不屬於這張表單草稿的即存設定 */
  children?: ReactNode;
}) {
  const [apiKey, setApiKey] = useState(config.api_keys["openrouter"] ?? "");
  const [tierModels, setTierModels] = useState<Record<string, string>>({
    ...SUGGESTED_TIER_MODELS,
    ...config.tier_models,
  });
  const [baseUrl, setBaseUrl] = useState(String(config.preferences["base_url"] ?? ""));
  const [modelMode, setModelMode] = useState(normalizeApiModelMode(config.preferences["api_model_mode"]));
  const [smartStatus, setSmartStatus] = useState<{
    model: string;
    freeDaily: { limit: number; remaining: number } | null;
  } | null>(null);
  const [smartRecommendations, setSmartRecommendations] =
    useState<SmartFreeRecommendationList>(EMPTY_RECOMMENDATIONS);
  // §15 新限免提示的整體開關；只有明確存 false 才關閉。
  const [notifyNewModels, setNotifyNewModels] = useState(
    config.preferences["smart_free_notify"] !== false,
  );
  // 穩定免費只挑 OpenRouter 的免費模型；自訂 base URL 時後端也一律照手動設定走
  const onOpenRouter = ["", "https://openrouter.ai/api/v1"].includes(baseUrl.trim().replace(/\/+$/, ""));
  const keyWarning = checkApiKey(apiKey, baseUrl);
  const [imageModel, setImageModel] = useState(String(config.preferences["image_model"] ?? ""));
  const [claudeCompatBaseUrl, setClaudeCompatBaseUrl] = useState(
    String(config.preferences["claude_base_url"] ?? ""),
  );
  const [claudeCompatKey, setClaudeCompatKey] = useState(config.api_keys["claude_compat"] ?? "");
  const [gmTier, setGmTier] = useState(String(config.preferences["gm_tier"] ?? "best"));
  const [maxRound, setMaxRound] = useState(String(config.preferences["max_round_speakers"] ?? 3));
  const [transport, setTransport] = useState(String(config.preferences["transport"] ?? "api"));
  const stableFree = transport === "api" && onOpenRouter && modelMode === "stable_free";
  const recommendedApiModel = transport === "api" && onOpenRouter && modelMode === "recommended";
  const fixedApiModel = stableFree || recommendedApiModel;
  const [permissionNotice, setPermissionNoticeState] = useState("");
  const noticeRef = useRef("");
  const [riskAccepted, setRiskAccepted] = useState(config.preferences["cli_risk_accepted"] === true);
  const [clis, setClis] = useState<CliInfo[] | null>(cachedClis());
  const catalogs = useModelCatalogs();
  const [customTiers, setCustomTiers] = useState<Record<string, boolean>>({});
  const [message, setMessage] = useState<SaveMessage | null>(null);
  const [saving, setSaving] = useState(false);
  // 同步守門：連按兩下送出時第二次 render 還沒發生，state 擋不住
  const savingRef = useRef(false);

  // 阻擋＝儲存中∥權限提示開著：兩者任一變動都依兩個 ref 重算後回報，不各自寫死 true／false
  function reportBlocking() {
    onBlockingChange(savingRef.current || noticeRef.current !== "");
  }

  function setSavingNow(next: boolean) {
    savingRef.current = next;
    setSaving(next);
    reportBlocking();
  }

  function setPermissionNotice(provider: string) {
    noticeRef.current = provider;
    setPermissionNoticeState(provider);
    reportBlocking();
  }
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

  // 監聽器只掛一次；onSaved 走 ref 取最新值，避免安裝中重掛掉事件
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
        void updateConfig({
          preferences: {
            [cliConnectedKey(event.payload.provider)]: event.payload.stage === "done",
          },
        })
          .then((saved) => onSavedRef.current(saved))
          .catch(() => {});
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

  async function installCli(provider: string, switchAccount = false) {
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
        switchAccount,
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
      setMessage({
        kind: "error",
        text: cooldown ? t("cliLoginCooldown", { secs: cooldown[1] }) : error,
      });
      if (!repeat) setInstallingCli(null);
      return;
    }

    // 換帳號流程一旦真的啟動就會登出，舊帳號當場失效：徽章轉灰，登入驗證過了輪詢再點亮。
    // 不轉灰的話，登入視窗被關掉時會停在「已連結 ✓」，實際上人已經被登出了。
    // 擺在 invoke 之後：冷卻中或啟動失敗會走上面的 catch，那些情況帳號沒被動到。
    if (switchAccount) {
      await updateConfig({
        preferences: { [cliConnectedKey(provider)]: false },
      })
        .then((saved) => onSavedRef.current(saved))
        .catch(() => {});
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
          void updateConfig({
            preferences: { [cliConnectedKey(provider)]: true },
          })
            .then((saved) => onSavedRef.current(saved))
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

  // 背景刷新換到新快取後前端才拿得到最新推薦／狀態；靠這把 tick 讓下面兩個查詢重跑。
  const [cacheTick, setCacheTick] = useState(0);
  useEffect(() => {
    const unlisten = listen(CACHE_UPDATED_EVENT, () => setCacheTick((tick) => tick + 1));
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  // 穩定免費與推薦模式都顯示「今日免費配額」（全帳號共用）。
  useEffect(() => {
    if (!fixedApiModel) {
      setSmartStatus(null);
      return;
    }
    invoke<{
      model: string;
      freeDaily: { limit: number; remaining: number } | null;
    }>("smart_free_status")
      .then((status) =>
        setSmartStatus(status.model ? { model: status.model, freeDaily: status.freeDaily } : null),
      )
      .catch(() => setSmartStatus(null));
  }, [fixedApiModel, cacheTick]);

  useEffect(() => {
    if (transport !== "api" || !onOpenRouter) {
      setSmartRecommendations(EMPTY_RECOMMENDATIONS);
      return;
    }
    invoke<SmartFreeRecommendationList>("smart_free_recommendations")
      .then(setSmartRecommendations)
      .catch(() => setSmartRecommendations(EMPTY_RECOMMENDATIONS));
  }, [transport, onOpenRouter, config, cacheTick]);

  function selectRecommendedModel(model: string) {
    setModelMode("recommended");
    setTierModels((previous) => ({
      ...previous,
      best: model,
      balanced: model,
      fast: model,
    }));
  }

  function recommendedModelSelected(model: string) {
    return (
      recommendedApiModel &&
      (["best", "balanced", "fast"] as const).every((tier) => tierModels[tier] === model)
    );
  }

  // 未儲存偵測：與 config 現值逐欄比對（比對值採 save() 相同的正規化），改幾欄算幾項
  const dirtyCount = [
    apiKey.trim() !== (config.api_keys["openrouter"] ?? ""),
    baseUrl.trim() !== String(config.preferences["base_url"] ?? ""),
    imageModel.trim() !== String(config.preferences["image_model"] ?? ""),
    claudeCompatBaseUrl.trim() !== String(config.preferences["claude_base_url"] ?? ""),
    claudeCompatKey.trim() !== (config.api_keys["claude_compat"] ?? ""),
    gmTier !== String(config.preferences["gm_tier"] ?? "best"),
    modelMode !== normalizeApiModelMode(config.preferences["api_model_mode"]),
    String(Math.max(1, Number(maxRound) || 3)) !==
      String(config.preferences["max_round_speakers"] ?? 3),
    transport !== String(config.preferences["transport"] ?? "api"),
    riskAccepted !== (config.preferences["cli_risk_accepted"] === true),
    notifyNewModels !== (config.preferences["smart_free_notify"] !== false),
    JSON.stringify(tierModels) !==
      JSON.stringify({ ...SUGGESTED_TIER_MODELS, ...config.tier_models }),
  ].filter(Boolean).length;

  useEffect(() => {
    onDirty(dirtyCount);
    return () => onDirty(0);
  }, [dirtyCount, onDirty]);

  // 「已儲存」留到玩家再動手改才消失；存檔成功後 N→0 不清，錯誤訊息留到下次存檔
  const previousDirty = useRef(dirtyCount);
  useEffect(() => {
    if (previousDirty.current === 0 && dirtyCount > 0) {
      setMessage((current) => (current?.kind === "ok" ? null : current));
    }
    previousDirty.current = dirtyCount;
  }, [dirtyCount]);

  // 視窗被外面直接收掉時別把阻擋狀態留在外框
  const blockingRef = useRef(onBlockingChange);
  blockingRef.current = onBlockingChange;
  useEffect(() => () => blockingRef.current(false), []);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    // 權限提示開著時底下欄位已停用，這裡再擋一次（Enter 送出等繞過按鈕的路徑）
    if (savingRef.current || noticeRef.current !== "") return;
    setMessage(null);
    if (transport !== "api" && !riskAccepted) {
      setMessage({ kind: "error", text: t("riskRequired") });
      return;
    }
    const preferences: Record<string, unknown> = {};
    const setPref = (key: string, value: unknown, unchanged: boolean) => {
      if (!unchanged) preferences[key] = value;
    };
    setPref(
      "base_url",
      baseUrl.trim(),
      baseUrl.trim() === String(config.preferences["base_url"] ?? ""),
    );
    setPref(
      "image_model",
      imageModel.trim(),
      imageModel.trim() === String(config.preferences["image_model"] ?? ""),
    );
    setPref(
      "claude_base_url",
      claudeCompatBaseUrl.trim(),
      claudeCompatBaseUrl.trim() === String(config.preferences["claude_base_url"] ?? ""),
    );
    setPref("transport", transport, transport === String(config.preferences["transport"] ?? "api"));
    setPref(
      "cli_risk_accepted",
      riskAccepted,
      riskAccepted === (config.preferences["cli_risk_accepted"] === true),
    );
    setPref("gm_tier", gmTier, gmTier === String(config.preferences["gm_tier"] ?? "best"));
    setPref(
      "api_model_mode",
      modelMode,
      modelMode === normalizeApiModelMode(config.preferences["api_model_mode"]),
    );
    setPref(
      "smart_free_notify",
      notifyNewModels,
      notifyNewModels === (config.preferences["smart_free_notify"] !== false),
    );
    const round = Math.max(1, Number(maxRound) || 3);
    setPref(
      "max_round_speakers",
      round,
      String(round) === String(config.preferences["max_round_speakers"] ?? 3),
    );
    const apiKeys: Record<string, string> = {};
    if (apiKey.trim() !== (config.api_keys["openrouter"] ?? "")) apiKeys.openrouter = apiKey.trim();
    if (claudeCompatKey.trim() !== (config.api_keys["claude_compat"] ?? "")) {
      apiKeys.claude_compat = claudeCompatKey.trim();
    }
    const tierPatch: Record<string, string> = {};
    for (const [key, value] of Object.entries(tierModels)) {
      if (value !== (config.tier_models[key] ?? "")) tierPatch[key] = value;
    }
    const patch: Record<string, unknown> = {};
    if (Object.keys(preferences).length > 0) patch.preferences = preferences;
    if (Object.keys(apiKeys).length > 0) patch.api_keys = apiKeys;
    if (Object.keys(tierPatch).length > 0) patch.tier_models = tierPatch;
    if (Object.keys(patch).length === 0) {
      setMessage({ kind: "ok", text: t("saved") });
      return;
    }
    setSavingNow(true);
    try {
      const saved = await updateConfig(patch);
      onSaved(saved);
      setMessage({ kind: "ok", text: t("saved") });
      const notice =
        transport !== "api" && saved.preferences[cliNoticeKey(transport)] !== true ? transport : "";
      // 先立提示再解除儲存中：阻擋狀態一路維持，不留讓人離開的空檔
      if (notice) setPermissionNotice(notice);
      setSavingNow(false);
    } catch (reason) {
      // 失敗保留草稿，恢復可編輯
      setMessage({ kind: "error", text: String(reason) });
      setSavingNow(false);
    }
  }

  // 看過就記下，同一家不再擋；寫失敗就當沒看過（下次再提醒，比默默吞掉好）
  async function ackPermissionNotice() {
    const provider = permissionNotice;
    setPermissionNotice("");
    try {
      onSaved(await updateConfig({ preferences: { [cliNoticeKey(provider)]: true } }));
    } catch {
      /* 記不起來只是下次再問一次，不打斷玩家 */
    }
  }

  const statusShown = message !== null || dirtyCount > 0;
  // 權限提示開著時背景整片不可聚焦（含捲動區尾端的即存設定與儲存列），焦點只留在提示裡
  const noticeOpen = permissionNotice !== "";

  return (
    <form id="ai-settings-form" onSubmit={save} className="settings-ai-form">
      <div className="settings-scroll" inert={noticeOpen}>
        {/* 儲存中或權限提示開著時整片欄位停用；提示畫在這個 fieldset 外，才按得到確認 */}
        <fieldset
          className="settings-form settings-fieldset"
          disabled={saving || noticeOpen}
        >
          <TransportChoice
            transport={transport}
            onTransport={setTransport}
            clis={clis}
            config={config}
            installingCli={installingCli}
            installProgress={installProgress}
            onInstall={(id, switchAccount) => void installCli(id, switchAccount)}
          />
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
          {/* OpenRouter 專屬欄位只在 API 直連時顯示，避免 CLI 使用者誤以為必填 */}
          {transport === "api" && (
            <>
              <label>
                {t("apiKeyLabel")}
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.currentTarget.value)}
                  placeholder={t("apiKeyPlaceholder")}
                />
              </label>
              {/* 貼錯的當下就講，不必等到發言才撞 401（那時的訊息還會把人導去 CLI 的重新驗證） */}
              {keyWarning && (
                <p className="field-warn" role="alert">
                  {t(keyWarning)}
                </p>
              )}
            </>
          )}
          {transport === "api" ? (
            <>
              <label>
                {t("imageModelLabel")}
                <input value={imageModel} onChange={(e) => setImageModel(e.currentTarget.value)} />
              </label>
              {onOpenRouter && (
                <fieldset className="transport-choice">
                  <legend>{t("modelModeLegend")}</legend>
                  {/* 今日免費配額是全帳號共用池，穩定免費與推薦模式都顯示 */}
                  {fixedApiModel && smartStatus && (
                    <p className="cli-version" role="status">
                      {smartStatus.freeDaily
                        ? t("smartFreeDailyLeft", {
                            remaining: smartStatus.freeDaily.remaining,
                            limit: smartStatus.freeDaily.limit,
                          })
                        : t("smartFreeUnlimited")}
                    </p>
                  )}
                  <div className="smart-free-recommendations">
                    <p className="smart-free-recommendation-title">{t("smartFreeStableTitle")}</p>
                    <label className="smart-free-recommendation">
                      <input
                        type="radio"
                        name="api-model-mode"
                        checked={stableFree}
                        onChange={() => setModelMode("stable_free")}
                      />
                      <span className="smart-free-recommendation-copy">
                        <strong>
                          {smartRecommendations.stable[0]?.label ?? t("smartFreeStableAuto")}
                        </strong>
                        {smartRecommendations.stable[0] ? (
                          <small>{recommendationReason(smartRecommendations.stable[0])}</small>
                        ) : (
                          <small>{t("smartFreeNoStable")}</small>
                        )}
                      </span>
                    </label>
                    {/* 第二名不自動送出，只供第一名當天失效時手動改用（點了＝固定該支） */}
                    {smartRecommendations.stable.slice(1).map((model) => (
                      <label key={model.model} className="smart-free-recommendation">
                        <input
                          type="radio"
                          name="api-model-mode"
                          checked={recommendedModelSelected(model.model)}
                          onChange={() => selectRecommendedModel(model.model)}
                        />
                        <span className="smart-free-recommendation-copy">
                          <strong>{model.label}</strong>
                          <small>{recommendationReason(model)}</small>
                        </span>
                      </label>
                    ))}
                    <p className="smart-free-recommendation-title">{t("smartFreeLimitedTitle")}</p>
                    {smartRecommendations.limited.length === 0 ? (
                      <p className="cli-version">{t("smartFreeNoLimited")}</p>
                    ) : (
                      smartRecommendations.limited.map((model) => (
                        <label key={model.model} className="smart-free-recommendation">
                          <input
                            type="radio"
                            name="api-model-mode"
                            checked={recommendedModelSelected(model.model)}
                            onChange={() => selectRecommendedModel(model.model)}
                          />
                          <span className="smart-free-recommendation-copy">
                            <strong>{model.label}</strong>
                            <span>{recommendationAvailability(model)}</span>
                            <small>{recommendationReason(model)}</small>
                          </span>
                        </label>
                      ))
                    )}
                    <label className="inline smart-free-notify-toggle">
                      <input
                        type="checkbox"
                        checked={notifyNewModels}
                        onChange={(e) => setNotifyNewModels(e.currentTarget.checked)}
                      />
                      {t("smartFreeNotifyLabel")}
                    </label>
                  </div>
                  <label className="inline">
                    <input
                      type="radio"
                      name="api-model-mode"
                      checked={modelMode !== "stable_free" && modelMode !== "recommended"}
                      onChange={() => setModelMode("manual")}
                    />
                    {t("manualModelOption")}
                  </label>
                </fieldset>
              )}
              {!fixedApiModel &&
                (["best", "balanced", "fast"] as const).map((tier) => (
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
          {/* 智慧免費自動挑模型，不走檔位→模型解析，這欄對它沒作用，藏起來 */}
          {!fixedApiModel && (
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
          )}
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
          {/* 智慧免費必然在 OpenRouter 預設站台（設了自訂 URL 就不算智慧免費），這欄對它沒意義 */}
          {transport === "api" && !fixedApiModel && (
            <label>
              {t("baseUrlLabel")}
              <input
                value={baseUrl}
                onChange={(e) => setBaseUrl(e.currentTarget.value)}
                placeholder="https://openrouter.ai/api/v1"
              />
            </label>
          )}
        </fieldset>
        {children}
      </div>
      {/* 每家 CLI 第一次啟用時擋一次：此時 CLI 還沒被叫起來，玩家先知道等一下的彈窗是誰在問 */}
      {permissionNotice && (
        <div
          className="modal-overlay"
          onClick={(event) => {
            // 只收這個提示，不能一路冒泡把設定視窗也關掉
            event.stopPropagation();
            void ackPermissionNotice();
          }}
        >
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
      <footer className="settings-foot" inert={noticeOpen}>
        {/* 固定約兩行高、超長內捲：可聚焦才能只用鍵盤捲完長錯誤 */}
        <div className="settings-foot-status" tabIndex={statusShown ? 0 : undefined}>
          {message && <span role={message.kind === "ok" ? "status" : "alert"}>{message.text}</span>}
          {dirtyCount > 0 && (
            <span className="settings-unsaved" role="status">
              {t("unsavedChanges", { n: dirtyCount })}
            </span>
          )}
        </div>
        {/* 兩種文案疊在同一格、寬度取最寬者：換字時按鈕不縮放、右邊的儲存鈕不位移 */}
        <button type="button" className="btn settings-back" disabled={saving} onClick={onBack}>
          <span className="settings-back-labels">
            <span aria-hidden={dirtyCount > 0}>{t("settingsBack")}</span>
            <span aria-hidden={dirtyCount === 0}>{t("settingsDiscard")}</span>
          </span>
        </button>
        <button
          type="submit"
          className="btn btn-primary settings-save"
          disabled={dirtyCount === 0 || saving}
        >
          {t("saveSettings")}
        </button>
      </footer>
    </form>
  );
}
