import { describe, expect, it } from "vitest";
import { OFFICIAL_ENDPOINTS, resolveEndpoints } from "./resolve";

const overrides = {
  VITE_TT_OPENROUTER_API: "http://127.0.0.1:9/api/v1",
  VITE_TT_OPENROUTER_AUTH: "http://127.0.0.1:9/auth",
  VITE_TT_MODELSCOPE_API: "http://127.0.0.1:9/ms/v1",
  VITE_TT_SILICONFLOW_API: "http://127.0.0.1:9/sf/v1",
  VITE_TT_GITHUB_API: "http://127.0.0.1:9/github",
};

describe("resolveEndpoints", () => {
  it("ignores VITE_TT_* overrides in a normal production build", () => {
    expect(resolveEndpoints("production", overrides)).toEqual(OFFICIAL_ENDPOINTS);
    expect(resolveEndpoints("development", overrides)).toEqual(OFFICIAL_ENDPOINTS);
  });

  it("accepts overrides only in e2e mode", () => {
    expect(resolveEndpoints("e2e", overrides)).toEqual({
      openrouterApi: overrides.VITE_TT_OPENROUTER_API,
      openrouterAuth: overrides.VITE_TT_OPENROUTER_AUTH,
      modelscopeApi: overrides.VITE_TT_MODELSCOPE_API,
      siliconflowApi: overrides.VITE_TT_SILICONFLOW_API,
      githubApi: overrides.VITE_TT_GITHUB_API,
    });
    expect(resolveEndpoints("e2e", {})).toEqual(OFFICIAL_ENDPOINTS);
  });
});
