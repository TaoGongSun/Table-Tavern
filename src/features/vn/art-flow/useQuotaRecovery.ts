import { useEffect, useState } from "react";
import { t } from "../../../i18n";

/** 顯示用的整數相對時間；不控制來源暫停或恢復。 */
export function quotaRecovery(reset: number | null, now = Date.now()): string {
  if (reset === null) return t("vnQuotaUnknown");
  const remaining = Math.max(0, reset - now);
  if (remaining >= 86_400_000) return t("vnQuotaTomorrow");
  if (remaining >= 3_600_000) return t("vnQuotaHours", { n: Math.round(remaining / 3_600_000) });
  return t("vnQuotaMinutes", { n: Math.max(1, Math.round(remaining / 60_000)) });
}

export function useQuotaRecovery(reset: number | null) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (reset === null) return;
    setNow(Date.now());
    const clock = setInterval(() => setNow(Date.now()), 60_000);
    return () => clearInterval(clock);
  }, [reset]);
  return quotaRecovery(reset, now);
}
