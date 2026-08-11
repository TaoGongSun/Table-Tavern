// AI 卡重構前端排程（A 拓撲）：盤點結果出爐後「怎麼發呼叫」的純邏輯——序列鏈＋有界並行
// 兩線並跑，外加限流重試。零 React／零 invoke 依賴，App.tsx 只管接線，排程與重試在這裡單獨測。

export const REFACTOR_PARALLEL_LIMIT = 4;

/**
 * A 拓撲排程：chain＝序列鏈（逐項 await，順序不變）；pool＝有界並行（同時在途數上限 limit，
 * 工人迴圈式逐項取件，不 Promise.all 一次全發）。
 *
 * 首發建快取：後端 system 提示詞快取要第一次呼叫落地才有效，預設（warmed=false）永遠先讓
 * 「唯一一條」呼叫獨自跑完才放行其餘——chain 非空就是 chain[0]（跑完後放行 pool 全部＋chain
 * 其餘並行）；chain 空則退而求其次讓 pool[0] 頂替（跑完後放行 pool 其餘）。
 * warmed=true：呼叫端保證快取已在同一 run 的更早呼叫（如盤點）建好，不必再獨跑——chain 與
 * pool 從一開始就並行開跑（chain 內部順序仍不變，只是不再等它跑完才放行 pool）。
 *
 * isCancelled()＝true 後不再「發新項」，已經在途的呼叫不受影響（由呼叫端另外 abort）。
 * run 的 contract：呼叫端保證它永不 reject（內部自行 try/catch），這裡不接 catch。
 * 全部項目都 settle 完才 resolve。
 */
export async function runRefactorCalls<T>(opts: {
  chain: T[];
  pool: T[];
  limit: number;
  isCancelled: () => boolean;
  run: (item: T) => Promise<void>;
  warmed?: boolean;
}): Promise<void> {
  const { chain, pool, limit, isCancelled, run, warmed = false } = opts;

  async function runChain(items: T[]): Promise<void> {
    for (const item of items) {
      if (isCancelled()) return;
      await run(item);
    }
  }

  async function runPool(items: T[]): Promise<void> {
    if (items.length === 0) return;
    let next = 0;
    const workerCount = Math.min(limit, items.length);
    const workers = Array.from({ length: workerCount }, async () => {
      while (next < items.length) {
        if (isCancelled()) return;
        await run(items[next++]);
      }
    });
    await Promise.all(workers);
  }

  if (warmed) {
    if (isCancelled()) return;
    await Promise.all([runChain(chain), runPool(pool)]);
    return;
  }

  if (chain.length > 0) {
    if (isCancelled()) return;
    await run(chain[0]);
    if (isCancelled()) return;
    await Promise.all([runChain(chain.slice(1)), runPool(pool)]);
  } else if (pool.length > 0) {
    if (isCancelled()) return;
    await run(pool[0]);
    if (isCancelled()) return;
    await runPool(pool.slice(1));
  }
}

/** 限流類錯誤判定：429／rate limit／overloaded（大小寫不拘，rate 與 limit 間容許任一字元）。 */
export function isRateLimitError(message: string): boolean {
  return /429|rate.?limit|overloaded/i.test(message);
}

/**
 * 單次退避重試：fn 失敗且判定為限流錯誤且尚未取消，等 delayMs 後重跑一次；其餘情況
 * （非限流錯誤、或已取消）原樣 throw。重試後第二次仍失敗一樣原樣 throw，不再重試。
 * delayMs 預設 15 秒（真打 API 用），測試傳 0 跳過真等待。
 */
export async function withRateLimitRetry<R>(
  fn: () => Promise<R>,
  isCancelled: () => boolean,
  delayMs = 15000,
): Promise<R> {
  try {
    return await fn();
  } catch (reason) {
    if (!isRateLimitError(String(reason)) || isCancelled()) throw reason;
  }
  await new Promise((resolve) => setTimeout(resolve, delayMs));
  return fn();
}
