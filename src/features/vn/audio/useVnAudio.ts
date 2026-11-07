// 播放器音訊：兩條循環音軌交叉淡化，音效一格一次。檔還沒落地就維持現在這首，等狀態事件再切。
import * as tauriCore from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import type { AudioView } from "../../../shared/contracts/backend-contracts";
import type { FlatCell } from "../vn-cells";
import { canStartSfx, desiredBgmKey, effectiveVolume, filePath, reachedSfx, type AudioFile } from "./audio-policy";

const FADE_MS = 1000;
const STATUS_EVENT = "vn-audio-status";

interface Status {
  worldId: string;
  kind: string;
  key: string;
  state: string;
}

function assetSrc(path: string): string {
  const convert = (tauriCore as { convertFileSrc?: (filePath: string) => string }).convertFileSrc;
  return convert ? convert(path) : "";
}

function makeAudio(): HTMLAudioElement | null {
  try {
    const audio = new Audio();
    audio.loop = true;
    return audio;
  } catch {
    return null;
  }
}

function start(audio: HTMLAudioElement, arm: (audio: HTMLAudioElement) => void) {
  try {
    const pending = audio.play();
    if (pending && typeof pending.catch === "function") void pending.catch(() => arm(audio));
  } catch {
    arm(audio);
  }
}

/** 實際指定給這條音軌的位址。比對用這個，不拿 key 充當播放身分。 */
const pinnedSrc = new WeakMap<HTMLAudioElement, string>();

function pin(audio: HTMLAudioElement, src: string) {
  pinnedSrc.set(audio, src);
  if (src) audio.src = src;
  else audio.removeAttribute("src");
}

function silence(audio: HTMLAudioElement) {
  audio.pause();
  pin(audio, "");
}

function isBgmTrack(
  audio: HTMLAudioElement,
  slots: [HTMLAudioElement, HTMLAudioElement] | null,
): boolean {
  return slots?.[0] === audio || slots?.[1] === audio;
}

export function useVnAudio({
  worldId,
  bgm,
  cells,
  flatIndex,
  lastEventKey,
  bgmVolume,
  sfxVolume,
  active,
  muted,
}: {
  worldId: string;
  /** 舞台摺到目前這格的配樂；null＝還沒寫過 */
  bgm: string[] | null;
  cells: FlatCell[];
  flatIndex: number;
  lastEventKey: string | null;
  bgmVolume: number;
  sfxVolume: number;
  /** 標題沒開、演出沒暫停、游標已定位。否則不播音效、也不登記已播。 */
  active: boolean;
  /** 勾了靜音：滑桿數字留著，實際音量為 0。 */
  muted: boolean;
}) {
  const files = useRef<AudioFile[]>([]);
  const [catalog, setCatalog] = useState(0);
  const slots = useRef<[HTMLAudioElement, HTMLAudioElement] | null>(null);
  const slot = useRef(0);
  const currentKey = useRef<string | null | undefined>(undefined);
  const fade = useRef(0);
  const played = useRef(new Set<string>());
  const liveSfx = useRef(new Set<HTMLAudioElement>());
  const lastSfxAt = useRef<number | null>(null);
  const blocked = useRef<HTMLAudioElement[]>([]);
  const bgmVol = useRef(bgmVolume);
  const sfxVol = useRef(sfxVolume);
  const mutedNow = useRef(muted);
  const activeNow = useRef(active);
  bgmVol.current = bgmVolume;
  sfxVol.current = sfxVolume;
  mutedNow.current = muted;
  activeNow.current = active;

  const arm = (audio: HTMLAudioElement) => {
    if (!blocked.current.includes(audio)) blocked.current.push(audio);
  };

  const dropInactiveSfx = () => {
    if (activeNow.current) return;
    blocked.current = blocked.current.filter((audio) => isBgmTrack(audio, slots.current));
  };

  useEffect(() => {
    const retry = () => {
      dropInactiveSfx();
      const waiting = blocked.current.splice(0);
      for (const audio of waiting) start(audio, arm);
    };
    window.addEventListener("pointerdown", retry);
    window.addEventListener("keydown", retry);
    return () => {
      window.removeEventListener("pointerdown", retry);
      window.removeEventListener("keydown", retry);
    };
  }, []);

  useEffect(() => {
    const first = makeAudio();
    const second = makeAudio();
    slots.current = first && second ? [first, second] : null;
    return () => {
      if (fade.current) cancelAnimationFrame(fade.current);
      first?.pause();
      second?.pause();
      for (const audio of liveSfx.current) audio.pause();
      liveSfx.current.clear();
      blocked.current = [];
    };
  }, []);

  useEffect(() => {
    let alive = true;
    // 切桌先丟掉上一桌的目錄與音軌。目錄載回前 key 即使同為 theme 也不能接著播。
    files.current = [];
    played.current = new Set();
    lastSfxAt.current = null;
    currentKey.current = undefined;
    blocked.current = [];
    if (fade.current) cancelAnimationFrame(fade.current);
    fade.current = 0;
    for (const audio of slots.current ?? []) silence(audio);
    for (const audio of liveSfx.current) audio.pause();
    liveSfx.current.clear();
    const load = () => {
      tauriCore.invoke<AudioView | null>("vn_audio_read", { worldId })
        .then((view) => {
          if (!alive) return;
          files.current = view?.files ?? [];
          setCatalog((n) => n + 1);
        })
        .catch(() => {});
    };
    load();
    const off = listen<Status>(STATUS_EVENT, ({ payload }) => {
      if (!alive || payload.worldId !== worldId || payload.state !== "done") return;
      load();
    });
    return () => {
      alive = false;
      void off.then((stop) => stop());
      for (const audio of slots.current ?? []) silence(audio);
      for (const audio of liveSfx.current) audio.pause();
      liveSfx.current.clear();
    };
  }, [worldId]);

  useEffect(() => {
    const pair = slots.current;
    if (!pair) return;
    const key = desiredBgmKey(bgm);
    const path = key ? filePath(files.current, "bgm", key) : null;
    // 檔還沒落地就維持現在這首。切桌時目錄已清空，這裡不會拿到上一桌的路徑。
    if (key && !path) return;
    const src = path ? assetSrc(path) : "";
    if (path && !src) return;
    const now = pair[slot.current];
    if (currentKey.current === key && pinnedSrc.get(now) === src) {
      if (key && fade.current === 0) now.volume = effectiveVolume(mutedNow.current, bgmVol.current);
      return;
    }
    currentKey.current = key;
    const from = now;
    const to = path ? pair[1 - slot.current] : null;
    if (to) {
      pin(to, src);
      to.loop = true;
      to.currentTime = 0;
      slot.current = 1 - slot.current;
      start(to, arm);
    }
    if (fade.current) cancelAnimationFrame(fade.current);
    const fromVol = from.volume;
    const begun = performance.now();
    const step = (nowTime: number) => {
      const t = Math.min(1, (nowTime - begun) / FADE_MS);
      // 靜音時淡出、淡入都是 0。取消後仍用這段的 t，不沿用勾下去之前抓住的音量。
      const audible = mutedNow.current ? 0 : 1;
      from.volume = fromVol * (1 - t) * audible;
      if (to) to.volume = effectiveVolume(mutedNow.current, bgmVol.current) * t;
      if (t < 1) {
        fade.current = requestAnimationFrame(step);
        return;
      }
      fade.current = 0;
      from.pause();
      if (!to) pin(from, "");
    };
    fade.current = requestAnimationFrame(step);
  }, [bgm, bgmVolume, muted, catalog, worldId]);

  useEffect(() => {
    const level = effectiveVolume(muted, bgmVolume);
    const pair = slots.current;
    // 淡出的那一軌也要立刻跟著，不能只改目前正在進來的這一軌。
    if (pair) {
      for (const audio of pair) {
        if (pinnedSrc.get(audio)) audio.volume = level;
      }
    }
    const sfxLevel = effectiveVolume(muted, sfxVolume);
    for (const audio of liveSfx.current) audio.volume = sfxLevel;
  }, [muted, bgmVolume, sfxVolume, catalog]);

  useEffect(() => {
    if (!active) {
      dropInactiveSfx();
      return;
    }
    for (const cell of reachedSfx(cells, flatIndex, lastEventKey)) {
      if (cell.cell.type !== "sfx" || played.current.has(cell.key)) continue;
      const path = filePath(files.current, "sfx", cell.cell.key);
      if (!path) continue;
      const src = assetSrc(path);
      if (!src) continue;
      // 同時兩聲、或離上一聲不到 250 毫秒：丟掉，但登記，避免停下來又補播。
      const now = performance.now();
      if (!canStartSfx(liveSfx.current.size, lastSfxAt.current, now)) {
        played.current.add(cell.key);
        continue;
      }
      let audio: HTMLAudioElement;
      try {
        audio = new Audio(src);
      } catch {
        continue;
      }
      played.current.add(cell.key);
      lastSfxAt.current = now;
      audio.volume = effectiveVolume(mutedNow.current, sfxVol.current);
      liveSfx.current.add(audio);
      audio.addEventListener("ended", () => liveSfx.current.delete(audio));
      start(audio, arm);
    }
  }, [cells, flatIndex, lastEventKey, catalog, worldId, active]);

  // 放開音效滑桿才試聽。靜音或這桌還沒有落地音效就不響。
  const previewSfx = () => {
    if (mutedNow.current) return;
    const path = files.current.find((file) => file.kind === "sfx")?.path;
    if (!path) return;
    const src = assetSrc(path);
    if (!src) return;
    let audio: HTMLAudioElement;
    try {
      audio = new Audio(src);
    } catch {
      return;
    }
    audio.volume = effectiveVolume(false, sfxVol.current);
    // 試聽不是劇情格：不佔連發名額、不登記已播，但要跟著靜音、切桌與卸載停掉。
    liveSfx.current.add(audio);
    audio.addEventListener("ended", () => liveSfx.current.delete(audio));
    start(audio, arm);
  };

  return { previewSfx };
}
