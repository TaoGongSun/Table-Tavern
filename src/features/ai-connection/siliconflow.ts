// 中國供應商（矽基流動）：API 通道的 base_url 指到這裡就算選了它；與 Rust transport::siliconflow::BASE_URL 同值。
export const SILICONFLOW_BASE_URL = "https://api.siliconflow.cn/v1";
export const SILICONFLOW_KEY_URL = "https://cloud.siliconflow.cn/account/ak";

export function isSiliconFlow(baseUrl: unknown): boolean {
  return String(baseUrl ?? "").trim().replace(/\/+$/, "") === SILICONFLOW_BASE_URL;
}
