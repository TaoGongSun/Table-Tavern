import { describe, expect, it } from "vitest";
import { setLang, t } from "../../i18n";
import {
  OPENROUTER_ONBOARDING_LANGS,
  OPENROUTER_ONBOARDING_MESSAGE_KEYS,
  openRouterOnboardingMessage,
} from "../../i18n/features/openrouter-onboarding";
import {
  OPENROUTER_ONBOARDING_ERROR_CODES,
  createOpenRouterPkce,
  openRouterOnboardingErrorKey,
  pkceChallengeForVerifier,
} from "./openrouter-onboarding";

describe("OpenRouter onboarding", () => {
  it("matches the RFC 7636 S256 example", async () => {
    const verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    await expect(pkceChallengeForVerifier(verifier)).resolves.toBe(
      "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
    );
  });

  it("creates verifier and challenge shapes accepted by the backend", async () => {
    const { verifier, challenge } = await createOpenRouterPkce();
    expect(verifier).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(challenge).toMatch(/^[A-Za-z0-9_-]{43}$/);
  });

  it("has every supplemental message in all ten UI languages", () => {
    expect(OPENROUTER_ONBOARDING_LANGS).toHaveLength(10);
    for (const lang of OPENROUTER_ONBOARDING_LANGS) {
      for (const key of OPENROUTER_ONBOARDING_MESSAGE_KEYS) {
        expect(openRouterOnboardingMessage(lang, key).trim().length).toBeGreaterThan(0);
      }
    }
  });

  it("maps every backend/frontend error code to an actionable localized key", () => {
    for (const code of OPENROUTER_ONBOARDING_ERROR_CODES) {
      const key = openRouterOnboardingErrorKey(code);
      for (const lang of OPENROUTER_ONBOARDING_LANGS) {
        expect(openRouterOnboardingMessage(lang, key).trim().length).toBeGreaterThan(0);
      }
    }
    expect(openRouterOnboardingErrorKey("unexpected failure")).toBe("onboardErrUnknown");
    expect(openRouterOnboardingErrorKey(new Error("openrouter_oauth_crypto"))).toBe(
      "onboardErrCrypto",
    );
  });

  it("routes supplemental messages through the global t() entry point", () => {
    setLang("en");
    expect(t("onboardConnectBtn")).toBe(openRouterOnboardingMessage("en", "onboardConnectBtn"));
    setLang("zh-TW");
  });
});
