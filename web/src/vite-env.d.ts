/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 端對端測試換成本機假端點；正式建置不設。 */
  readonly VITE_TT_OPENROUTER_API?: string;
  readonly VITE_TT_OPENROUTER_AUTH?: string;
  readonly VITE_TT_GITHUB_API?: string;
}

/** 建置時由 mvu-engine-plugin.ts 包出的 MVU 共用核心（桌面版沙盒同一份原始碼） */
declare module "virtual:mvu-engine" {
  export function createMvuCore(
    env: import("@desktop/features/mvu-engine/host-contract").MvuCoreEnv,
  ): import("@desktop/features/mvu-engine/host-contract").MvuCore;
}
