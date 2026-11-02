// 房間協定 v1 的型別（規格：.ai/plans/multiplayer-room-protocol.md）。純型別，核心與各殼共用。
export type RoomStatus = "lobby" | "playing" | "transition" | "closed";
export interface RoomSettings {
  maxPlayers: number;
  actionTimeoutSec: number | null;
  shuffleOrder: boolean;
  allowCustomCards: boolean;
}
export interface SceneSettings {
  route: string;
  model: string;
  modelLabel: string;
}
export type RoundKind = "open" | "scene_open" | "play";
export type RoundPhase = "acting" | "queued" | "generating" | "held";
export type ActStatus = "pending" | "drafting" | "submitted" | "skipped";
export interface RosterEntry {
  memberId: string;
  cardId: string;
  status: ActStatus;
  text: string | null;
}
export interface RoundView {
  round: number;
  kind: RoundKind;
  scene: number;
  phase: RoundPhase;
  roster: RosterEntry[];
  deadline: number | null;
  pausedRemainingMs: number | null;
  dispatchId: string | null;
  hold: PublicExecutorError | null;
}
export interface PublicCard {
  cardId: string;
  name: string;
  color: string | null;
  avatar: string | null;
  blurb: string;
  playable: boolean;
  hidden: boolean;
  claimedBy: string | null;
}
export type SystemMarker = "scene_summary" | "card_arrival" | "person_arrival" | "state_update";
export type Speaker =
  | { kind: "gm" }
  | { kind: "npc"; cardId: string; name: string }
  | { kind: "player"; memberId: string; cardId: string; name: string }
  | { kind: "system"; marker: SystemMarker };
export interface StoryPart {
  round: number;
  part: number;
  speaker: Speaker;
  text: string;
  interrupted: boolean;
  at: number;
}
export interface ActionItem {
  memberId: string;
  cardId: string;
  name: string;
  kind: "act" | "watch";
  text?: string;
  watchReason?: "skipped" | "timeout" | "host_resolved";
}
export type ExecutorErrorKind = "refusal" | "quota" | "tool_format" | "provider_unreachable" | "scene_full" | "not_ready" | "other";
export type RemoteAction = "retry" | "switch_model" | "switch_route" | "continue" | "scene_advance";
export interface ExecutorError {
  kind: ExecutorErrorKind;
  code: string;
  message: string;
  actions: RemoteAction[];
  resetsAt: number | null;
}
export interface PublicExecutorError {
  kind: ExecutorErrorKind;
  actions: RemoteAction[];
  resetsAt: number | null;
}
export interface QuotaReport {
  route: string;
  routeLabel: string;
  window: "rolling" | "daily" | "weekly" | "credits" | "unknown";
  state: "ok" | "near" | "exhausted" | "unknown";
  usedRatio: number | null;
  resetsAt: number | null;
  creditsLeft: string | null;
  observedAt: number;
}
export interface RouteOffer {
  route: string;
  label: string;
  models: Array<{ id: string; label: string; laxity: 1 | 2 | 3 | null }>;
}
export interface Capabilities {
  routes: RouteOffer[];
  current: SceneSettings;
  maxRoundSpeakers: number;
}
export type RemoteCommand =
  | { kind: "retry" }
  | { kind: "switch_model"; model: string }
  | { kind: "switch_route"; route: string; model: string }
  | { kind: "abort" }
  | { kind: "set_next"; route: string; model: string }
  | { kind: "refresh_quota" };
export interface Dispatch {
  dispatchId: string;
  round: number;
  kind: RoundKind;
  scene: number;
  settings: SceneSettings;
  actions: ActionItem[];
  players: Array<{ cardId: string; memberId: string; name: string; connected: boolean }>;
  arrivals: string[];
  departures: Array<{ cardId: string; mode: "npc" | "absent" }>;
}
export type Job =
  | { jobId: string; kind: "scene_advance"; scene: number; nextSettings: SceneSettings }
  | { jobId: string; kind: "card_create"; memberId: string; name: string; blurb: string };
export interface InflightReport {
  dispatch: { dispatchId: string; state: "running" | "done" | "failed"; lastEndedPart: number | null } | null;
  job: { jobId: string; state: "running" | "done" } | null;
}
export interface ExecutorView {
  label: string;
  online: boolean;
  since: number;
  keepAwake: boolean | null;
  modelLabel: string | null;
}

export type ErrorCode =
  | "E_BAD_FRAME" | "E_VERSION" | "E_AUTH_REQUIRED" | "E_AUTH_FAILED" | "E_ROOM_NOT_FOUND" | "E_ROOM_CLOSED"
  | "E_BANNED" | "E_KICKED" | "E_ROOM_FULL" | "E_NAME_INVALID" | "E_TEXT_INVALID" | "E_FORBIDDEN"
  | "E_ROUND_STALE" | "E_ROUND_NOT_ACTING" | "E_STATE_CONFLICT" | "E_SEAT_TAKEN" | "E_SEAT_REQUIRED"
  | "E_EXECUTOR_OFFLINE" | "E_EXECUTOR_TIMEOUT" | "E_GEN_TIMEOUT" | "E_PAIR_CODE_INVALID" | "E_PAIR_LOCKED"
  | "E_MODEL_NOT_OFFERED" | "E_SETTING_INVALID" | "E_RATE_LIMITED" | "E_DISPATCH_UNKNOWN"
  | "E_SESSION_REPLACED" | "E_EXECUTOR_REPLACED" | "E_INTERNAL";

// 欄位內容由核心各處理器再驗（validate.ts）；信封只保證 t 在白名單、p 是物件
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export interface Frame { t: string; id?: string; p: any }
export interface StoredEvent { seq: number; at: number; type: string; payload: Record<string, unknown> }
export type ConnRole = "member" | "executor";
export interface ConnInfo { id: string; role: ConnRole }

// ── 房間狀態（整份可序列化；殼一筆交易落地） ──
export interface Member {
  id: string;
  dev: string; // SHA-256(roomCode:deviceId) 十六進位
  nickname: string;
  cardId: string | null;
  seat: number; // 入座序
  banned: boolean;
  arriving: boolean; // 中途入座、下一次開 acting 才進 roster
}
export interface RoundState extends RoundView {
  batch: ActionItem[];
  arrivals: string[];
  dispatch: Dispatch | null;
  partsEnded: number; // 已落地的段數；part 依序，第 n 段 end 後為 n+1
  speaker: Speaker | null; // 進行中那一段的說話者（gen.part.end 不帶 speaker）
  genAt: number; // 最後一則 gen.* 的時間，15 分鐘無進度轉 held
}
export interface PendingJob { job: Job; reply: { conn: string; id?: string } | null }
export interface RoomState {
  code: string;
  status: RoomStatus;
  settings: RoomSettings;
  hostId: string | null;
  payerId: string | null;
  scene: number;
  sceneSettings: SceneSettings | null;
  nextSettings: SceneSettings | null;
  transition: { jobId: string; since: number; progressAt: number; stalled: boolean } | null;
  createdAt: number;
  lastActive: number;
  idleDays: number;
  closedAt: number | null;
  joinHash: string;
  execHash: string | null;
  exec: { label: string; online: boolean; since: number; lastSeen: number; keepAwake: boolean | null; busy: boolean; caps: Capabilities | null; quota: QuotaReport | null };
  execConn: string | null;
  seq: number;
  roundNo: number;
  sceneStartSeq: number;
  summarySeqs: number[];
  seeded: boolean;
  round: RoundState | null;
  members: Member[];
  memberNo: number;
  seatNo: number;
  cards: PublicCard[]; // 換名冊時整個換新陣列（殼靠參照判斷要不要重寫）
  drafts: Record<string, string>;
  pair: { hash: string; expiresAt: number; grantHost: boolean } | null;
  pairFails: number;
  pairLockUntil: number;
  departures: Array<{ cardId: string; mode: "npc" | "absent" }>;
  jobs: PendingJob[];
  doneJobs: string[];
  lastDoneDispatch: string | null;
  lastError?: { round: number; error: ExecutorError } | null; // 最近一筆錯誤細節，只給出額度者（重連後還看得到）；舊房間沒有這欄
  ctls: Record<string, { conn: string; id?: string; at: number; cmd: RemoteCommand; settings?: SceneSettings; hold?: PublicExecutorError | null }>;
  conns: Record<string, { role: ConnRole; who: string | null; at: number }>; // who＝memberId／"exec"；null＝還沒 hello
}

/** 不落地的記憶體值：串流緩衝、分片建房緩衝、頻率桶。DO 被逐出就遺失（協定 §4.3 已容許）。 */
export interface Live {
  stream: { round: number; part: number; speaker: Speaker; text: string; announced: boolean } | null;
  seed: { next: number; cards: PublicCard[] | null; history: StoryPart[] } | null;
  buckets: Record<string, { start: number; n: number }>;
}

export interface CoreCtx {
  now: number;
  rng: (bytes: number) => Uint8Array;
  live: Live;
  store: { eventsAfter(seq: number, limit: number): Promise<StoredEvent[]>; eventsBefore(seq: number, limit: number): Promise<StoredEvent[]> };
}
export type CoreInput =
  | { kind: "frame"; conn: ConnInfo; raw: string }
  | { kind: "connect" | "disconnect"; conn: ConnInfo }
  | { kind: "alarm" }
  | { kind: "http"; route: "create"; body: unknown; code: string; origin: string; idleDays: number }
  | { kind: "http"; route: "export"; deviceId: string };
/** 寄給執行端的訊框用這個收件者，由殼的 ExecutorPort 轉送（v1 WebSocket、v2 程序內） */
export const TO_EXECUTOR = "@executor";
export interface CoreOutput {
  state: RoomState | null;
  events: StoredEvent[];
  sends: Array<{ to: string; frame: Frame }>;
  close: Array<{ conn: string; code: number }>;
  alarmAt: number | null;
  deleteRoom?: true;
  http?: { status: number; body: unknown };
}
