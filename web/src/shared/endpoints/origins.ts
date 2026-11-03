// 網頁版對外連的所有端點（模型供應商、GitHub）。只有 e2e 模式才換成本機假端點（見 resolve.ts）。
import { resolveEndpoints } from "./resolve";

const endpoints = resolveEndpoints(import.meta.env.MODE, import.meta.env);

export const OPENROUTER_API = endpoints.openrouterApi;
export const OPENROUTER_AUTH = endpoints.openrouterAuth;
export const MODELSCOPE_API = endpoints.modelscopeApi;
export const SILICONFLOW_API = endpoints.siliconflowApi;
export const GITHUB_API = endpoints.githubApi;

export const REPO = "TaoGongSun/Table-Tavern";
/** 沒有正式版、或 API 抓不到時的退路。 */
export const RELEASES_PAGE = `https://github.com/${REPO}/releases`;
/** 桌面版安裝檔的 pCloud 公開資料夾：GitHub 在中國連不上，簡中與中國供應商模式排在最前；換版只換檔、網址不變。 */
export const PCLOUD_PAGE = "https://u.pcloud.link/publink/show?code=kZAMsSJZtvgA74prulBpSdWuKP0vfLIYvFT7";
