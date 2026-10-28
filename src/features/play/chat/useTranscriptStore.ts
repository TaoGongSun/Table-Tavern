// 逐字稿存取：這一幕畫面上的事件清單、本地改動追蹤、重讀與追加。
// 世代由 useChatController 持有（單一來源），這裡只讀；世代一變就換一份新的改動追蹤。
import { MutableRefObject, useCallback, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TranscriptEvent } from "../../../shared/contracts/backend-contracts";
import { hasContent } from "./undo-stack";

/** 一個換桌／換幕世代的本地改動追蹤：rev 每次改動換進畫面加一、inFlight 在途數、waiters 等在途清空的重讀 */
interface WriteTrack {
  generation: number;
  rev: number;
  inFlight: number;
  waiters: (() => void)[];
}

export function useTranscriptStore({
  worldId,
  scene,
  generation,
  actionIdRef,
  appendFailed,
  onAppended,
  onError,
}: {
  worldId: string;
  scene: number;
  /** controller 的換桌／換幕世代 */
  generation: MutableRefObject<number>;
  /** 目前玩家動作的 id：追加的每一則都蓋上 */
  actionIdRef: MutableRefObject<string | null>;
  /** 這一輪有追加回錯（收尾時重讀用） */
  appendFailed: MutableRefObject<boolean>;
  /** 追加成功、換進畫面之後（清掉收回堆疊） */
  onAppended: () => void;
  onError: (message: string) => void;
}) {
  const [events, setEventsState] = useState<TranscriptEvent[]>([]);
  // 逐字稿的同步副本：每次更新先算進這裡再交給 React，讓事件／回合開始時讀得到「已 setEvents 但還沒渲染」的長度
  const eventsLive = useRef<TranscriptEvent[]>([]);
  const setEvents = useCallback(
    (next: TranscriptEvent[] | ((previous: TranscriptEvent[]) => TranscriptEvent[])) => {
      eventsLive.current = typeof next === "function" ? next(eventsLive.current) : next;
      setEventsState(eventsLive.current);
    },
    [],
  );
  // 本地改動追蹤（見 beginWrite／reload）按世代各一份：換世代換新的一份並叫醒舊世代的等待者，
  // 舊桌永不回應的落檔拖不住新桌的重讀，舊桌晚到的收尾也碰不到新桌的計數
  const writes = useRef<WriteTrack>({ generation: generation.current, rev: 0, inFlight: 0, waiters: [] });
  if (writes.current.generation !== generation.current) {
    const old = writes.current;
    writes.current = { generation: generation.current, rev: 0, inFlight: 0, waiters: [] };
    for (const wake of old.waiters.splice(0)) wake();
  }

  // 畫面上的逐字稿除了重讀，還會被本地改動換掉：先落檔、回來再把結果換進畫面（追加、收回、卡片寫入換表）。
  // 重讀從發出到回來之間只要有本地改動完成或還在途，讀到的那份就可能比畫面舊（蓋回舊表、吃掉剛追加的事件），
  // 或跟函式型追加疊成兩份——那份不套用、等改動落定再讀。rev 在每次本地改動換進畫面時加一
  /** 本地改動開始（落檔前呼叫）；回傳的收尾在結果換進畫面之後呼叫，重複呼叫只算一次，只動發起當時那個世代的計數 */
  const beginWrite = useCallback(() => {
    const track = writes.current;
    track.inFlight += 1;
    let open = true;
    return () => {
      if (!open) return;
      open = false;
      track.inFlight -= 1;
      track.rev += 1;
      if (track.inFlight === 0) for (const wake of track.waiters.splice(0)) wake();
    };
  }, []);

  const hydrate = useCallback((transcript: TranscriptEvent[]) => {
    writes.current.rev += 1;
    setEvents(transcript);
  }, []);

  // 重讀：同時只跑一趟（同世代），在途期間再被要求就在這趟回來後補讀一次；呼叫端拿到的 promise
  // 等最後一趟套用完才 resolve。讀的期間有本地改動（見 beginWrite）就整份作廢重讀，在途改動先等它落定
  const reloading = useRef<{ generation: number; again: boolean; promise: Promise<void> } | null>(null);
  const reload = useCallback(() => {
    const started = generation.current;
    const running = reloading.current;
    if (running && running.generation === started) {
      running.again = true;
      return running.promise;
    }
    const job = { generation: started, again: false, promise: Promise.resolve() };
    job.promise = (async () => {
      try {
        for (;;) {
          job.again = false;
          const track = writes.current;
          if (track.generation !== started) return;
          while (track.inFlight > 0) {
            await new Promise<void>((wake) => track.waiters.push(wake));
            if (generation.current !== started) return;
          }
          const rev = track.rev;
          const transcript = await invoke<TranscriptEvent[]>("read_transcript", { worldId, scene });
          if (generation.current !== started) return;
          if (track.rev !== rev || track.inFlight > 0) continue;
          setEvents(transcript);
          if (!job.again) return;
        }
      } finally {
        if (reloading.current === job) reloading.current = null;
      }
    })();
    reloading.current = job;
    return job.promise;
  }, [worldId, scene]);

  // 卡片寫入的落檔在卡片 controller 的佇列裡，這裡只在確認後拿到結果：換掉同一物件或同 id 那一則（只會有一則）。
  // 第一次被寫入的舊事件原本沒 id，畫面若在這期間重讀過就認不出是哪一則（同時間同正文的舊事件可能不只一則），
  // 不猜：照樣只換同一物件，另外重讀一次拿後端的權威結果
  const replaceEvent = useCallback(
    (previous: TranscriptEvent, next: TranscriptEvent) => {
      writes.current.rev += 1;
      setEvents((list) =>
        list.map((event) =>
          event === previous || (event.id !== undefined && event.id === next.id) ? next : event,
        ),
      );
      if (previous.id === undefined) reload().catch((reason: unknown) => onError(String(reason)));
    },
    [reload, onError],
  );

  // started＝發起這一輪時的世代，由回合一路傳下來：換桌之後才呼叫也不會把新世代當成自己的
  const appendEvent = useCallback(
    // turn＝GM 回合的一部分（main 正文／state_update 變動紀錄）：後端以 (turn_id, turn_part) 冪等落檔，
    // main 掛上回合算好的變數表；characterTurn＝這則是那次角色聊天呼叫的回覆，後端憑它認世界書落地成功
    async (
      event: TranscriptEvent,
      started: number,
      turn?: { turnId: string; part: "main" | "state_update" },
      characterTurn?: string,
    ) => {
      // 空白事件一律不落地（stream-failure-visible）：AI 失敗時故事不該多出一則看不見的
      // 回合，它還會進下一次呼叫的歷史把模型帶偏。後端 API 路徑已擋，這裡是 CLI 路徑
      // 與任何未來新路徑的保險，錯誤碼與後端同一個
      // 帶已知標頭代碼的事件（點名）本文可以是空的
      if (!hasContent(event)) {
        throw new Error("AI_EMPTY_RESPONSE: 空白回合不寫進故事");
      }
      // 用後端回傳的那份（快照已補好）進畫面：收回後要復原時，送回去的事件才帶著當時的
      // 檯面值，狀態欄跟著回到那一刻
      let stamped: TranscriptEvent;
      // 動作中寫下的每一則都蓋上這個動作的 id：後端換幕容量預測靠它切出每個動作的回覆量
      const action = actionIdRef.current;
      const endWrite = beginWrite();
      try {
        try {
          stamped = await invoke<TranscriptEvent>("append_transcript", {
            worldId,
            scene,
            event: action ? { ...event, action_id: action } : event,
            turnId: turn?.turnId ?? null,
            turnPart: turn?.part ?? null,
            characterTurn: characterTurn ?? null,
          });
        } catch (reason) {
          appendFailed.current = true;
          throw reason;
        }
        if (generation.current !== started) return;
        setEvents((previous) => [...previous, stamped]);
      } finally {
        endWrite();
      }
      // 桌上一有新內容，收回的那幾句就不能再放回去了——位置已經被後面的話蓋掉
      onAppended();
    },
    [worldId, scene, beginWrite, actionIdRef, appendFailed, generation, setEvents, onAppended],
  );

  return { events, eventsLive, setEvents, beginWrite, hydrate, reload, replaceEvent, appendEvent };
}
