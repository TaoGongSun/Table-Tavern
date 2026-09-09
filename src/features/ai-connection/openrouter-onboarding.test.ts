import { describe, expect, it } from "vitest";
import type { Lang } from "../../i18n";
import {
  OPENROUTER_ONBOARDING_ERROR_CODES,
  createOpenRouterPkce,
  openRouterOnboardingCopy,
  openRouterOnboardingError,
  pkceChallengeForVerifier,
} from "./openrouter-onboarding";

const LANGS: Lang[] = ["zh-TW", "zh-CN", "en", "ja", "ko", "es", "pt-BR", "de", "fr", "ru"];

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

  it("has complete onboarding copy and actionable errors for all ten UI languages", () => {
    for (const lang of LANGS) {
      const copy = openRouterOnboardingCopy(lang);
      for (const field of [
        copy.title,
        copy.intro,
        copy.freeNote,
        copy.connect,
        copy.connecting,
        copy.browserHint,
        copy.manualSummary,
        copy.manualIntro,
        copy.saveKey,
        copy.savingKey,
        copy.cliHint,
      ]) {
        expect(field.trim().length).toBeGreaterThan(0);
      }
      for (const code of OPENROUTER_ONBOARDING_ERROR_CODES) {
        expect(openRouterOnboardingError(lang, code).trim().length).toBeGreaterThan(0);
      }
      expect(openRouterOnboardingError(lang, "unexpected failure").trim().length).toBeGreaterThan(0);
    }
  });

  it("unwraps frontend Error messages before mapping them", () => {
    expect(openRouterOnboardingError("en", new Error("openrouter_oauth_crypto"))).toBe(
      openRouterOnboardingCopy("en").errors.openrouter_oauth_crypto,
    );
  });
});
