// 網頁版對外連的所有端點（OpenRouter、GitHub）。只有 e2e 模式才換成本機假端點（見 resolve.ts）。
import { resolveEndpoints } from "./resolve";

const endpoints = resolveEndpoints(import.meta.env.MODE, import.meta.env);

export const OPENROUTER_API = endpoints.openrouterApi;
export const OPENROUTER_AUTH = endpoints.openrouterAuth;
export const GITHUB_API = endpoints.githubApi;

export const REPO = "TaoGongSun/Table-Tavern";
/** 沒有正式版、或 API 抓不到時的退路。 */
export const RELEASES_PAGE = `https://github.com/${REPO}/releases`;
