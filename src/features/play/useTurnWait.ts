// 匯入、撤銷匯入、貼開場白、套用重構在後端整段持整桌獨占。回合進行中按下時，前端先等整個回合收尾
// （isTurnRunning() 回 false：GM 正文、狀態事件、角色回覆都已落檔，刷新跑完 finally 才清）才送後端，
// 不能以 AI 串流結束為準——後端在串流結束就放開許可，前端隨後才寫 GM 旁白，先送會讓舊回覆落在變更後的桌。
// 後端獨占仍是最後一道防線。不能先拿後端獨占再等，那會擋住回合落檔。
// run 讓同一桌的這類操作同時只跑一次（等待中再按直接忽略），交給操作一組工具：
// - backend 包住那一次後端呼叫：有回合在跑就亮 waiting 並等它結束，讓操作所在的位置顯示「等目前的回覆
//   結束後執行」；等待期間不再算數（換桌、卸載）就不送出、丟 TurnWaitAborted，run 收掉它。
// - live() 問「這次操作還算數嗎」：換桌、元件卸載，或之後又開始新的一次，就回 false。已送出的後端
//   操作照常在原桌完成，但完成後每個 await 邊界都要先問 live()，不算數就不回寫畫面、不刷新、不關面板。
// 停止生成的入口不受影響。
import { useCallback, useEffect, useRef, useState } from "react";

export interface TurnWaitOp {
  live: () => boolean;
  backend: <R>(call: () => Promise<R>) => Promise<R>;
}

/** 等回合期間操作已不算數：沒送後端就收掉 */
export class TurnWaitAborted extends Error {
  constructor() {
    super("turn wait aborted");
    this.name = "TurnWaitAborted";
  }
}

// busy 是同步 ref、清除時沒有通知，用短間隔輪詢；回合本身動輒數秒，這點延遲看不出來
const IDLE_POLL_MS = 50;

async function waitForIdle(isTurnRunning: () => boolean, live: () => boolean) {
  while (isTurnRunning()) {
    if (!live()) throw new TurnWaitAborted();
    await new Promise((resolve) => setTimeout(resolve, IDLE_POLL_MS));
  }
  if (!live()) throw new TurnWaitAborted();
}

export function useTurnWait(isTurnRunning: () => boolean, scope: string) {
  const [busy, setBusy] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const mounted = useRef(true);
  const scopeRef = useRef(scope);
  // 進行中那次操作的識別；null＝沒有。換桌時清掉，舊操作從此不算數，新桌也不被它擋住
  const active = useRef<number | null>(null);
  const nextToken = useRef(0);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      active.current = null;
    };
  }, []);

  useEffect(() => {
    if (scopeRef.current === scope) return;
    scopeRef.current = scope;
    active.current = null;
    setBusy(false);
    setWaiting(false);
  }, [scope]);

  /** 同一桌已有一次在跑就不執行 fn、回 undefined；等待中被終止也回 undefined */
  const run = useCallback(
    async <T>(fn: (op: TurnWaitOp) => Promise<T>): Promise<T | undefined> => {
      if (active.current !== null) return undefined;
      const token = ++nextToken.current;
      const origin = scopeRef.current;
      active.current = token;
      const live = () => mounted.current && active.current === token && scopeRef.current === origin;
      const backend = async <R>(call: () => Promise<R>): Promise<R> => {
        setWaiting(isTurnRunning());
        try {
          await waitForIdle(isTurnRunning, live);
          return await call();
        } finally {
          if (live()) setWaiting(false);
        }
      };
      setBusy(true);
      try {
        return await fn({ live, backend });
      } catch (reason) {
        if (reason instanceof TurnWaitAborted) return undefined;
        throw reason;
      } finally {
        if (live()) {
          active.current = null;
          setBusy(false);
        }
      }
    },
    [isTurnRunning],
  );

  /** 不管桌別的版本：給已由換桌互斥（runTableOp）保護、途中可能自己換到新桌的匯入用；
   *  只在卸載時終止（丟 TurnWaitAborted） */
  const backend = useCallback(
    async <R>(call: () => Promise<R>): Promise<R> => {
      setWaiting(isTurnRunning());
      try {
        await waitForIdle(isTurnRunning, () => mounted.current);
        return await call();
      } finally {
        if (mounted.current) setWaiting(false);
      }
    },
    [isTurnRunning],
  );

  return { busy, waiting, run, backend };
}
