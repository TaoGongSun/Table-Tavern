// VN 播放器的格來源：正式格（整幕重抓 read_vn_cells）＋串流暫定格（節流 parse_vn_text）＋兩者換手。
// 換手規則（方案 §2）：串流結束先凍結暫定格繼續顯示，等含新事件的正式格重抓回來，
// 才在同一次更新裡換成正式格並交給呼叫端對位游標；確定沒落檔的凍結段才撤掉。
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
  TranscriptEvent,
  VnCell,
  VnEventCells,
  VnText,
} from "../../shared/contracts/backend-contracts";
import { eventKeyOf, flatten, pendingCells, type FlatCell } from "./vn-cells";

const THROTTLE_MS = 150;

export interface Snapshot {
  generation: number;
  cells: VnCell[];
  tail: boolean;
}

interface Streams {
  /** 串流已結束、等正式格的段（依時間序） */
  frozen: Snapshot[];
  live: Snapshot | null;
}

export function useVnCells({
  worldId,
  scene,
  events,
  generating,
  streamText,
  onLanded,
  onError,
}: {
  worldId: string;
  scene: number;
  events: TranscriptEvent[];
  generating: { id: string; kind: string } | null;
  streamText: string;
  /** 暫定段換成正式事件的那一刻（與格的更新同一批），呼叫端據此對位游標 */
  onLanded: (snapshot: Snapshot, eventKey: string, formal: VnCell[]) => void;
  onError: (message: string) => void;
}) {
  const [formal, setFormal] = useState<VnEventCells[]>([]);
  /** 正式格屬於哪一幕（換幕清空後、新幕格到之前是 null）：播放器靠它在換幕時重新定位 */
  const [formalScene, setFormalScene] = useState<number | null>(null);
  const [streams, setStreams] = useState<Streams>({ frozen: [], live: null });
  const formalRef = useRef(formal);
  const streamsRef = useRef(streams);
  const fetchGen = useRef(0);
  const latest = useRef<Promise<void>>(Promise.resolve());
  const streamGen = useRef(0);
  const landed = useRef(-1);
  const callbacks = useRef({ onLanded, onError });
  callbacks.current = { onLanded, onError };

  const putStreams = (next: Streams) => {
    streamsRef.current = next;
    setStreams(next);
  };

  const apply = useCallback((result: VnEventCells[]) => {
    const known = new Set(formalRef.current.map(eventKeyOf));
    const fresh = result
      .map((event, position) => ({ event, key: eventKeyOf(event, position) }))
      .filter(({ event, key }) => event.kind !== "player" && !event.recap && !known.has(key));
    formalRef.current = result;
    setFormal(result);
    const { frozen, live } = streamsRef.current;
    const waiting = live ? [...frozen, live] : frozen;
    waiting.slice(0, fresh.length).forEach((snapshot, n) => {
      landed.current = Math.max(landed.current, snapshot.generation);
      callbacks.current.onLanded(snapshot, fresh[n].key, fresh[n].event.cells);
    });
    // 凍結段在它之後發出的重抓裡仍沒對到事件＝停止或失敗沒落檔，撤掉；還在長的段留著
    const keepLive = live !== null && fresh.length <= frozen.length;
    if (fresh.length > 0 || frozen.length > 0) {
      putStreams({ frozen: [], live: keepLive ? live : null });
    }
  }, []);

  /** 重抓整幕；resolve 時最新一次重抓已裝好（被後來的重抓蓋過就等後來那次） */
  const refresh = useCallback((): Promise<void> => {
    const mine = ++fetchGen.current;
    if (!worldId) return Promise.resolve();
    const run = (async () => {
      try {
        const result = await invoke<VnEventCells[] | null>("read_vn_cells", { worldId, scene });
        if (mine !== fetchGen.current) return latest.current;
        apply(result ?? []);
        setFormalScene(scene);
      } catch (reason) {
        if (mine === fetchGen.current) callbacks.current.onError(String(reason));
      }
    })();
    latest.current = run;
    return run;
  }, [worldId, scene, apply]);

  useEffect(() => {
    formalRef.current = [];
    setFormal([]);
    setFormalScene(null);
    putStreams({ frozen: [], live: null });
  }, [worldId, scene]);

  useEffect(() => {
    void refresh();
  }, [refresh, events]);

  // 每段串流一個世代（generating 每輪都是新物件）；上一段還在顯示就凍結並重抓確認落檔
  useEffect(() => {
    streamGen.current += 1;
    const { frozen, live } = streamsRef.current;
    if (live) {
      putStreams({ frozen: [...frozen, live], live: null });
      void refresh();
    }
  }, [generating, refresh]);

  const text = useRef({ streamText, generating, parsed: 0, seq: 0 });
  text.current = { ...text.current, streamText, generating };
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const parse = useCallback(async () => {
    timer.current = null;
    const { streamText: raw, generating: who } = text.current;
    const generation = streamGen.current;
    if (!who || !raw || landed.current >= generation) return;
    const seq = ++text.current.seq;
    text.current.parsed = raw.length;
    const source = who.id === "" ? { kind: "gm" } : { kind: "character", self_id: who.id };
    try {
      const result = await invoke<VnText | null>("parse_vn_text", { worldId, source, text: raw });
      const stale = seq !== text.current.seq || generation !== streamGen.current;
      if (!result || stale || landed.current >= generation) return;
      const live = { generation, cells: result.cells, tail: result.tail_tentative };
      putStreams({ ...streamsRef.current, live });
    } catch (reason) {
      callbacks.current.onError(String(reason));
    }
  }, [worldId]);

  useEffect(() => {
    if (!generating || !streamText) {
      text.current.parsed = 0;
      return;
    }
    const newline = streamText.slice(text.current.parsed).includes("\n");
    if (timer.current !== null && !newline) return;
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => void parse(), newline ? 0 : THROTTLE_MS);
  }, [generating, streamText, parse]);

  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );

  const cells = useMemo<FlatCell[]>(() => {
    const tentative = streams.live ? [...streams.frozen, streams.live] : streams.frozen;
    return [
      ...flatten(formal),
      ...tentative.flatMap((snapshot) => pendingCells(snapshot.generation, snapshot.cells)),
    ];
  }, [formal, streams]);

  const last = formal.length - 1;
  return {
    cells,
    lastEventKey: last >= 0 ? eventKeyOf(formal[last], last) : null,
    formalScene,
    tail: streams.live?.tail ?? false,
    /** 目前這段串流已解析出暫定格 */
    streaming: streams.live !== null,
    refresh,
  };
}
