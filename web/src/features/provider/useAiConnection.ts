// 目前用哪家供應商、哪支模型，以及對應的 session。兩種 session 的 hook 都常駐（hook 不能條件呼叫），
// 沒被選中的那邊不發任何請求。
import { useCallback, useEffect, useState } from "react";
import type { Lang } from "../../i18n";
import type { OAuthCallback } from "../openrouter/oauth";
import { useOpenRouterSession, type OpenRouterSession } from "../openrouter/useOpenRouterSession";
import type { AiSession } from "./ai-session";
import { initialProvider, loadModelChoice, rememberModelChoice, rememberProvider } from "./provider-choice";
import { CN_PROVIDERS, isCn, type CnProviderId, type ProviderId } from "./providers";
import { useKeyedSession, type KeyedSession } from "./useKeyedSession";

export interface AiConnection {
  provider: ProviderId;
  choose: (id: ProviderId) => void;
  /** 中國供應商的手選模型；null＝自動（寫死名單） */
  model: string | null;
  chooseModel: (model: string | null) => void;
  openrouter: OpenRouterSession;
  keyed: KeyedSession;
  /** 目前選中的那一家 */
  session: AiSession;
}

export function useAiConnection(callback: OAuthCallback | null, lang: Lang): AiConnection {
  // OpenRouter 授權回來＝玩家選了 OpenRouter
  const [provider, setProvider] = useState<ProviderId>(() => (callback ? "openrouter" : initialProvider(lang)));
  const [models, setModels] = useState<Record<CnProviderId, string | null>>(() => ({
    modelscope: loadModelChoice("modelscope"),
    siliconflow: loadModelChoice("siliconflow"),
  }));
  const cnId: CnProviderId = isCn(provider) ? provider : "modelscope";
  const openrouter = useOpenRouterSession(callback, provider === "openrouter");
  const keyed = useKeyedSession(CN_PROVIDERS[cnId], models[cnId]);

  useEffect(() => {
    if (callback) rememberProvider("openrouter");
  }, [callback]);

  const choose = useCallback((id: ProviderId) => {
    rememberProvider(id);
    setProvider(id);
  }, []);

  const chooseModel = useCallback(
    (model: string | null) => {
      rememberModelChoice(cnId, model);
      setModels((current) => ({ ...current, [cnId]: model }));
    },
    [cnId],
  );

  // 貼上金鑰也算選了這一家：之後換語系不會被預設值帶走
  const adoptKey = keyed.adoptKey;
  const adoptAndRemember = useCallback(
    async (key: string) => {
      const ok = await adoptKey(key);
      if (ok) rememberProvider(cnId);
      return ok;
    },
    [adoptKey, cnId],
  );
  const keyedSession: KeyedSession = { ...keyed, adoptKey: adoptAndRemember };

  return {
    provider,
    choose,
    model: isCn(provider) ? models[cnId] : null,
    chooseModel,
    openrouter,
    keyed: keyedSession,
    session: provider === "openrouter" ? openrouter : keyedSession,
  };
}
