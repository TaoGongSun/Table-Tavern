import { afterEach, expect, it } from "vitest";
import { currentLang, setLang, t } from "../../../i18n";
import { quotaRecovery } from "./useQuotaRecovery";

const original = currentLang();
afterEach(() => setLang(original));
it("uses rounded relative hours/minutes, tomorrow, and no dates or seconds", () => {
  setLang("zh-TW");
  const now = Date.UTC(2030, 0, 1);
  expect(quotaRecovery(now + 4.4 * 3_600_000, now)).toBe("約 4 小時後恢復");
  expect(quotaRecovery(now + 39.6 * 60_000, now)).toBe("約 40 分鐘後恢復");
  expect(quotaRecovery(now + 25 * 3_600_000, now)).toBe("明天恢復");
  expect(quotaRecovery(now + 10_000, now)).toBe("約 1 分鐘後恢復");
  expect(quotaRecovery(null, now)).toBe(t("vnQuotaUnknown"));
});
it("uses the same localized relative template in all ten languages", () => {
  for (const lang of ["zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"] as const) {
    setLang(lang);
    expect(quotaRecovery(4 * 3_600_000, 0)).toBe(t("vnQuotaHours", { n: 4 }));
    expect(quotaRecovery(40 * 60_000, 0)).toBe(t("vnQuotaMinutes", { n: 40 }));
    expect(quotaRecovery(25 * 3_600_000, 0)).toBe(t("vnQuotaTomorrow"));
  }
});

it("uses singular and plural full units and lowercase fragments", () => {
  setLang("en");
  expect(quotaRecovery(3_600_000, 0)).toBe("available again in about 1 hour");
  expect(quotaRecovery(60_000, 0)).toBe("available again in about 1 minute");
  expect(quotaRecovery(2 * 3_600_000, 0)).toBe("available again in about 2 hours");
  expect(quotaRecovery(40 * 60_000, 0)).toBe("available again in about 40 minutes");
  for (const lang of ["en", "es", "pt-BR", "de", "fr"] as const) {
    setLang(lang);
    expect(quotaRecovery(3_600_000, 0)[0]).toBe(quotaRecovery(3_600_000, 0)[0].toLowerCase());
  }
});
