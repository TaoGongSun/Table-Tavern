// 端點解析（純函式，vite.config.ts 的 CSP 與執行時共用同一份）。只有 e2e 模式（`vite build --mode e2e`，
// web/e2e/run.mjs）才接受 VITE_TT_* 覆寫成本機假端點；其他模式一律官方網址，環境變數再怎麼設都不理。
export interface Endpoints {
  openrouterApi: string;
  openrouterAuth: string;
  githubApi: string;
}

export const OFFICIAL_ENDPOINTS: Endpoints = {
  openrouterApi: "https://openrouter.ai/api/v1",
  openrouterAuth: "https://openrouter.ai/auth",
  githubApi: "https://api.github.com",
};

export const E2E_MODE = "e2e";

export function resolveEndpoints(mode: string, env: Record<string, string | undefined>): Endpoints {
  if (mode !== E2E_MODE) return OFFICIAL_ENDPOINTS;
  return {
    openrouterApi: env.VITE_TT_OPENROUTER_API ?? OFFICIAL_ENDPOINTS.openrouterApi,
    openrouterAuth: env.VITE_TT_OPENROUTER_AUTH ?? OFFICIAL_ENDPOINTS.openrouterAuth,
    githubApi: env.VITE_TT_GITHUB_API ?? OFFICIAL_ENDPOINTS.githubApi,
  };
}
