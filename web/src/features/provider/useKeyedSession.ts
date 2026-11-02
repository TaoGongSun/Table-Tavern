// 中國供應商的 session：玩家貼的金鑰（一家一格，只送去那家自己的 apiBase）、寫死名單或手選模型。
// 沒有每日次數可查（魔搭是魔粒、矽基流動只有每分鐘限速）：額度固定 unknown、不擋送出。
import { useCallback, useEffect, useMemo, useState } from "react";
import type { MsgKey } from "../../i18n";
import type { QuotaState } from "../funnel/quota";
import { FailoverRuntime } from "../openrouter/failover";
import type { ApiDeps } from "../openrouter/openrouter-api";
import type { AiSession } from "./ai-session";
import { FixedPool, fetchModelIds } from "./fixed-pool";
import { keyName, read, write } from "./provider-choice";
import type { CnProvider, CnProviderId } from "./providers";

export interface KeyedSession extends AiSession {
  /** i18n 鍵（cnKeyInvalid、keySessionOnly）；null＝沒有要說的。 */
  notice: MsgKey | null;
  /** 存下金鑰；驗證沒過回 false。 */
  adoptKey: (key: string) => Promise<boolean>;
}

const UNKNOWN: QuotaState = { kind: "unknown" };
const NO_HEADERS = {};
const noDaily = async () => null;
const noop = () => {};
const noopAsync = async () => {};

export function useKeyedSession(provider: CnProvider, model: string | null): KeyedSession {
  const deps = useMemo<ApiDeps>(() => ({ fetch: (...args) => fetch(...args), apiBase: provider.apiBase }), [provider]);
  // 這次開站後貼過或登出過的金鑰（存不進瀏覽器時也靠它在本分頁有效）；沒動過就讀瀏覽器裡存的
  const [held, setHeld] = useState<Partial<Record<CnProviderId, string | null>>>({});
  const [notice, setNotice] = useState<MsgKey | null>(null);
  const id = provider.id;
  const apiKey = id in held ? (held[id] ?? null) : read(keyName(id))?.trim() || null;
  // 換家、換模型、換金鑰：選模狀態整個換新（在途回合用的是自己開頭取定的那一份）
  const pool = useMemo(() => new FixedPool(provider, deps, model), [provider, deps, model, apiKey]);
  const runtime = useMemo(() => new FailoverRuntime(), [provider, model, apiKey]);

  useEffect(() => setNotice(null), [id]);

  const adoptKey = useCallback(
    async (raw: string) => {
      const key = raw.trim();
      if (!key) return false;
      if (provider.validateOnSave && (await fetchModelIds(deps, key)) === "unauthorized") {
        setNotice("cnKeyInvalid");
        return false;
      }
      const stored = write(keyName(provider.id), key);
      setHeld((current) => ({ ...current, [provider.id]: key }));
      setNotice(stored ? null : "keySessionOnly");
      return true;
    },
    [provider, deps],
  );

  const logout = useCallback(() => {
    write(keyName(id), null);
    setHeld((current) => ({ ...current, [id]: null }));
    setNotice(null);
  }, [id]);

  return {
    provider: id,
    apiKey,
    quota: UNKNOWN,
    deps,
    headers: NO_HEADERS,
    rateLimit: "per-model",
    pool,
    runtime,
    dailyCheck: noDaily,
    refreshQuota: noopAsync,
    quotaEvent: noop,
    logout,
    notice,
    adoptKey,
  };
}
