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

/** 建置時由 mvu-zod-engine-plugin.ts 給：桌面同一支 MVU bundle（文字）與 QuickJS wasm（base64） */
declare module "virtual:mvu-quickjs-assets" {
  export const bundle: string;
  export const wasm: string;
}

/** 建置時由 mvu-zod-engine-plugin.ts 打成 classic IIFE 的 zod 引擎原始碼（文字），交給不透明來源的 iframe 建 Worker */
declare module "virtual:mvu-zod-engine" {
  const source: string;
  export default source;
}
