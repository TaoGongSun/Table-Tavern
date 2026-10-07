// 登入狀態、PKCE 回呼、今日免費次數，以及選模用的 FreePool／FailoverRuntime（同一把金鑰共用）。
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { OPENROUTER_API, OPENROUTER_AUTH } from "../../shared/endpoints/origins";
import { nextQuota, type QuotaEvent, type QuotaState } from "../funnel/quota";
import { FailoverRuntime } from "./failover";
import { FreePool } from "./free-pool";
import { browserStorage, clearKey, loadKey, saveKey } from "./key-store";
import { beginAuthorization, completeAuthorization, type OAuthCallback } from "./oauth";
import { fetchFreeDaily, type ApiDeps } from "./openrouter-api";

export interface OpenRouterSession {
  apiKey: string | null;
  connecting: boolean;
  /** i18n 鍵（oauth_*、keySessionOnly）；null＝沒有要說的。 */
  notice: string | null;
  quota: QuotaState;
  deps: ApiDeps;
  pool: FreePool;
  runtime: FailoverRuntime;
  connect: () => Promise<void>;
  adoptKey: (key: string) => void;
  logout: () => void;
  refreshQuota: () => Promise<void>;
  quotaEvent: (event: QuotaEvent) => void;
}

/** 同一組回呼只換一次金鑰（StrictMode 會把 effect 跑兩次，兩次拿到的是同一個 callback 物件）。 */
let exchangeInFlight: { callback: OAuthCallback; promise: Promise<string> } | null = null;

export function useOpenRouterSession(callback: OAuthCallback | null): OpenRouterSession {
  const deps = useMemo<ApiDeps>(() => ({ fetch: (...args) => fetch(...args), apiBase: OPENROUTER_API }), []);
  const [apiKey, setApiKey] = useState<string | null>(() => loadKey(browserStorage("local")));
  const [connecting, setConnecting] = useState(callback !== null);
  const [notice, setNotice] = useState<string | null>(null);
  const [quota, setQuota] = useState<QuotaState>({ kind: "unknown" });
  const pool = useMemo(() => new FreePool(deps), [deps]);
  const runtime = useMemo(() => new FailoverRuntime(), []);
  const keyRef = useRef(apiKey);
  keyRef.current = apiKey;
  /** 授權世代：登出或改用貼上的金鑰時推進，晚到的 OAuth 交換結果不採用、不寫 storage。 */
  const authGenRef = useRef(0);

  const quotaEvent = useCallback((event: QuotaEvent) => setQuota((state) => nextQuota(state, event)), []);

  const adopt = useCallback(
    (key: string) => {
      const stored = saveKey(browserStorage("local"), key);
      runtime.invalidate();
      pool.invalidate();
      setApiKey(key);
      setNotice(stored ? null : "keySessionOnly");
    },
    [runtime, pool],
  );

  useEffect(() => {
    if (!callback) return;
    if (exchangeInFlight?.callback !== callback) {
      const session = browserStorage("session");
      exchangeInFlight = {
        callback,
        promise: session ? completeAuthorization(callback, session, deps) : Promise.reject(new Error("oauth_state")),
      };
    }
    const generation = authGenRef.current;
    let live = true;
    const current = () => live && authGenRef.current === generation;
    exchangeInFlight.promise
      .then((key) => current() && adopt(key))
      .catch((error: unknown) => current() && setNotice(error instanceof Error ? error.message : "oauth_exchange"))
      .finally(() => current() && setConnecting(false));
    return () => {
      live = false;
    };
  }, [callback, deps, adopt]);

  const refreshQuota = useCallback(async () => {
    const key = keyRef.current;
    if (!key) return;
    const daily = await fetchFreeDaily(deps, key);
    if (keyRef.current === key) quotaEvent({ type: "key-info", daily });
  }, [deps, quotaEvent]);

  useEffect(() => {
    if (apiKey) void refreshQuota();
  }, [apiKey, refreshQuota]);

  const connect = useCallback(async () => {
    const session = browserStorage("session");
    if (!session) {
      setNotice("oauth_crypto");
      return;
    }
    try {
      window.location.assign(await beginAuthorization(window.location, session, OPENROUTER_AUTH));
    } catch {
      setNotice("oauth_crypto");
    }
  }, []);

  /** 玩家自己貼的金鑰：作廢進行中的 OAuth 交換，免得晚到的結果蓋掉它。 */
  const adoptPasted = useCallback(
    (key: string) => {
      authGenRef.current += 1;
      setConnecting(false);
      adopt(key);
    },
    [adopt],
  );

  const logout = useCallback(() => {
    authGenRef.current += 1;
    setConnecting(false);
    clearKey(browserStorage("local"));
    runtime.invalidate();
    pool.invalidate();
    setApiKey(null);
    setNotice(null);
    quotaEvent({ type: "logout" });
  }, [runtime, pool, quotaEvent]);

  return { apiKey, connecting, notice, quota, deps, pool, runtime, connect, adoptKey: adoptPasted, logout, refreshQuota, quotaEvent };
}
