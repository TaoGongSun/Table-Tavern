// 殼雙緩衝的狀態轉移（純函式，.ai/plans/shell-update-flash.md）。任何時刻最多兩支 iframe：
// front＝正在顯示；pending＝背面在載的新殼，等它的 ready（或逾時）才翻上來。

export interface FrameSlot {
  /** 殼指紋＝iframe key＝文件 token */
  key: string;
  /** 掛載時就給的 srcdoc；同一個 key 的文件不再換（絕不改既有元素的 srcdoc） */
  doc: string;
  /** 顯示對象（桌、桌世代、殼所屬的卡）：不同就硬切，不雙緩衝 */
  identity: string;
}

export interface FramesState {
  front: FrameSlot | null;
  pending: FrameSlot | null;
  /** 鎖定期起點（宿主時刻）；換 pending 不重算，逾時從這裡算 */
  lockedAt: number | null;
}

export type FramesAction =
  | { type: "shell"; slot: FrameSlot | null; now: number }
  | { type: "ready"; key: string }
  | { type: "timeout" };

export const EMPTY_FRAMES: FramesState = { front: null, pending: null, lockedAt: null };

function settled(front: FrameSlot | null): FramesState {
  return { front, pending: null, lockedAt: null };
}

export function framesReducer(state: FramesState, action: FramesAction): FramesState {
  switch (action.type) {
    case "shell": {
      const { slot } = action;
      if (slot === null) return state.front === null && state.pending === null ? state : EMPTY_FRAMES;
      const { front, pending } = state;
      // 首開、重開面板：直接當 front，不等任何訊號（＝原本的單 iframe 直繪）
      if (front === null) return settled(slot);
      // 回到正在顯示的那份（例：收回上一句）：丟掉背面那支
      if (front.key === slot.key) return pending === null ? state : settled(front);
      if (pending?.key === slot.key) return state;
      // 換桌、還原、切卡：硬切
      if (front.identity !== slot.identity) return settled(slot);
      return { front, pending: slot, lockedAt: state.lockedAt ?? action.now };
    }
    case "ready":
      return state.pending?.key === action.key ? settled(state.pending) : state;
    case "timeout":
      return state.pending === null ? state : settled(state.pending);
  }
}

/** 鎖定期總上限：到時強制翻到最新的 pending（只保證不再等，不保證內容已畫好） */
export const FLIP_TIMEOUT_MS = 1500;

/**
 * 宿主訊息監聽看的現況（ShellFrames 每次落定後寫入）：哪支是 front、哪支在背面、是否在鎖定期。
 * 存元素、用到時才取 contentWindow 比對 event.source。
 */
export interface RegistrySlot {
  key: string;
  frame: HTMLIFrameElement | null;
}

export interface ShellRegistry {
  front: RegistrySlot | null;
  pending: RegistrySlot | null;
  locked: boolean;
}

export function createShellRegistry(): ShellRegistry {
  return { front: null, pending: null, locked: false };
}

/** 這份殼的 iframe 視窗 */
export function slotWindow(slot: RegistrySlot | null): Window | null {
  return slot?.frame?.contentWindow ?? null;
}

function from(slot: RegistrySlot | null, source: unknown, token: unknown): boolean {
  const win = slotWindow(slot);
  return slot !== null && win !== null && source === win && token === slot.key;
}

/** 卡片送來的行動（input）：只收目前 front、而且不在鎖定期 */
export function acceptsInput(registry: ShellRegistry, source: unknown, token: unknown): boolean {
  return !registry.locked && from(registry.front, source, token);
}

/** 卡片存設定（storage，整份快照）：有 pending 只收 pending 的，否則只收 front 的——舊殼晚寫不能蓋掉新殼 */
export function acceptsStorage(registry: ShellRegistry, source: unknown, token: unknown): boolean {
  return from(registry.pending ?? registry.front, source, token);
}
