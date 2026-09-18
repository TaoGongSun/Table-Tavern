import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "../i18n";
import { AppConfig } from "../shared/contracts/backend-contracts";

// 後端每小時背景刷新換到新快取後廣播；設定頁與此 banner 都聽它重查（見 smart_free/mod.rs）。
export const CACHE_UPDATED_EVENT = "smart-free-cache-updated";

interface NewModel {
  model: string;
  label: string;
}

interface Props {
  config: AppConfig;
  onConfigSaved: (config: AppConfig) => void;
  onOpenSettings: (tab: "ai") => void;
}

// §15 新限免提示：非阻塞浮層。開 App／設定變動時比對後端「上次看過之後才出現」的限時推薦，
// 玩家按改用／查看／略過任一個都記為看過，之後不再重複跳。整體開關與去重都在後端。
export function SmartFreeNewModelBanner({ config, onConfigSaved, onOpenSettings }: Props) {
  const [models, setModels] = useState<NewModel[]>([]);

  const transport = String(config.preferences["transport"] ?? "api");
  const baseUrl = String(config.preferences["base_url"] ?? "")
    .trim()
    .replace(/\/+$/, "");
  const onOpenRouter = ["", "https://openrouter.ai/api/v1"].includes(baseUrl);
  const hasKey = Boolean((config.api_keys["openrouter"] ?? "").trim());
  const active = transport === "api" && onOpenRouter && hasKey;

  const query = useCallback(() => {
    invoke<NewModel[]>("smart_free_new_models")
      .then(setModels)
      .catch(() => setModels([]));
  }, []);

  // 開 App／設定變動時查一次；掛載時跑的第一次背景刷新可能還沒完成，故也在快取更新事件後重查。
  // 先掛好監聽再做首查，避免「首查讀到舊快取→刷新 emit→監聽還沒掛上」的窄競速窗。
  useEffect(() => {
    if (!active) {
      setModels([]);
      return;
    }
    let stop: (() => void) | undefined;
    let cancelled = false;
    void listen(CACHE_UPDATED_EVENT, () => query()).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }
      stop = unlisten;
      query();
    });
    return () => {
      cancelled = true;
      stop?.();
    };
  }, [active, config, query]);

  if (models.length === 0) return null;

  const names = models.map((model) => model.label).join("、");
  const ids = models.map((model) => model.model);

  async function markSeen() {
    setModels([]);
    try {
      await invoke("smart_free_dismiss_recommendations", { models: ids });
    } catch {
      // 記不下去就下次再提醒，不擋玩家。
    }
  }

  async function useNext() {
    const model = models[0].model;
    // 先記為看過再改 config：改 config 會重跑查詢，去重若還沒落地會讓提示短暫重跳。
    await markSeen();
    const next: AppConfig = {
      ...config,
      tier_models: { ...config.tier_models, best: model, balanced: model, fast: model },
      preferences: { ...config.preferences, api_model_mode: "recommended" },
    };
    try {
      await invoke("write_config", { config: next });
      onConfigSaved(next);
    } catch {
      // 寫失敗就維持原設定，只收掉提示。
    }
  }

  function view() {
    onOpenSettings("ai");
    void markSeen();
  }

  return (
    <div className="smart-free-new-banner" role="status">
      <div className="smart-free-new-copy">
        <strong>{t("smartFreeNewTitle")}</strong>
        <span>{t("smartFreeNewBody", { model: names })}</span>
      </div>
      <div className="smart-free-new-actions">
        <button type="button" onClick={() => void useNext()}>
          {t("smartFreeNewUse")}
        </button>
        <button type="button" onClick={view}>
          {t("smartFreeNewView")}
        </button>
        <button type="button" className="ghost" onClick={() => void markSeen()}>
          {t("smartFreeNewDismiss")}
        </button>
      </div>
    </div>
  );
}
