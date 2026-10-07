/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 端對端測試換成本機假端點；正式建置不設。 */
  readonly VITE_TT_OPENROUTER_API?: string;
  readonly VITE_TT_OPENROUTER_AUTH?: string;
  readonly VITE_TT_GITHUB_API?: string;
}
