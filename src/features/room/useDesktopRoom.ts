// 電腦這台在房間裡的身分與連線（room-host-desktop-play）：放在 app 最外層，跟著「房間開著」而不是「房間畫面開著」——
// 編輯卡、看世界書、回大廳都不斷線〔§2.1〕。進房時電腦自己用 grantHost 碼入座成房主＋出額度者（交給手機後就不再自己拿）；
// 桌上有玩家卡就自動認領它（伺服器保留給這台裝置，朋友認不走）。
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState } from "react";
import type { RoomSession } from "@party/features/room/useRoomSession";
import { parseInvite, type Invite } from "@party/features/connection/invite";
import { t } from "../../i18n";
import { pairCodeOf, readRoomStatus, waitRoomStatus, type RoomStatus } from "./room-status";

interface Identity {
  deviceId: string;
  inviteUrl: string;
  phoneHost: boolean;
}

export interface Join {
  key: number;
  invite: Invite;
  device: string;
  /** 房間伺服器來源（房主面板重生邀請連結用） */
  origin: string;
}

/** 發一張新的 grantHost 碼並等它出現在狀態裡；`phone`＝要交給手機 */
async function freshPairLink(phone: boolean): Promise<string | null> {
  const before = (await readRoomStatus()).payerLink;
  await invoke("room_pair_code", { phone });
  const status = await waitRoomStatus((s) => s.payerLink !== null && s.payerLink !== before);
  return status?.payerLink ?? null;
}

/** 房主自己的玩家卡（進房前那張；房間期間存在 solo_player_card_id）：電腦自動認領它，暱稱也用它的名字 */
async function hostCard(worldId: string): Promise<{ id: string; name: string } | null> {
  const state = await invoke<{ player_card_id?: string | null; player_card_ids?: string[]; solo_player_card_id?: string | null }>("read_state", { worldId });
  const id = state.solo_player_card_id ?? ((state.player_card_ids ?? []).length === 0 ? state.player_card_id : null) ?? null;
  if (!id) return null;
  const card = await invoke<{ name: string }>("read_character", { worldId, characterId: id }).catch(() => null);
  return card ? { id, name: card.name } : null;
}

export type DesktopRoomCtl = ReturnType<typeof useDesktopRoom>;

/** `worldId`：房間開著（或剛從開房／重開進來）的那張桌，沒有＝null */
export function useDesktopRoom(worldId: string | null, deps: { cast: unknown; playerId: string | null; onPaused: () => void }) {
  const [status, setStatus] = useState<RoomStatus | null>(null);
  const [join, setJoin] = useState<Join | null>(null);
  const [room, setRoom] = useState<RoomSession | null>(null);
  const [failed, setFailed] = useState(false);
  const [notice, setNotice] = useState("");
  const [handoff, setHandoff] = useState<string | null>(null);
  const [nickname, setNickname] = useState(t("roomHostNickname"));
  const [mine, setMine] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const seatTried = useRef(false);
  const reentered = useRef(false);

  // 換桌（或房間關了）：連線與這間房的暫存全部重來
  useEffect(() => {
    setStatus(null);
    setJoin(null);
    setRoom(null);
    setFailed(false);
    setNotice("");
    setHandoff(null);
    setMine(null);
    seatTried.current = false;
    reentered.current = false;
    if (!worldId) return;
    let live = true;
    const poll = () => void readRoomStatus(worldId).then((s) => live && setStatus(s), () => {});
    poll();
    const timer = setInterval(poll, 2000);
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, [worldId]);

  const connect = useCallback(
    async (takeHost: boolean) => {
      if (!worldId) return;
      setFailed(false);
      const online = await waitRoomStatus((s) => s.active && s.online, 20_000);
      if (!online) return setFailed(true);
      const identity = await invoke<Identity>("room_identity");
      const card = await hostCard(worldId).catch(() => null);
      if (card) {
        setMine(card.id);
        setNickname(card.name);
      }
      const link = takeHost || !identity.phoneHost ? await freshPairLink(false) : null;
      const code = pairCodeOf(link);
      const invite = parseInvite(code ? `${identity.inviteUrl}&p=${code}` : identity.inviteUrl);
      if (!invite) return setFailed(true);
      setHandoff(null);
      const origin = new URL(identity.inviteUrl).origin;
      setJoin((previous) => ({ key: (previous?.key ?? 0) + 1, invite, device: identity.deviceId, origin }));
    },
    [worldId],
  );

  useEffect(() => {
    if (worldId) void connect(false);
  }, [worldId, connect]);

  // 房主在 CastRail 匯入、改、刪卡之後：大家看到的名冊跟著更新
  const { cast, playerId } = deps;
  useEffect(() => {
    if (!worldId) return;
    void invoke("room_roster_refresh").catch(() => {});
    // 開房後才建／匯入的玩家卡：也算電腦自己的，還沒入座就自動認領
    void hostCard(worldId).then((card) => card && setMine(card.id), () => {});
  }, [worldId, cast, playerId]);

  // 伺服器上的房間被清掉了（閒置過期）：同一張桌開新房，並告訴房主要重發邀請
  useEffect(() => {
    if (!worldId || !status || status.active || !status.gone || reentered.current) return;
    reentered.current = true;
    void invoke<RoomStatus>("room_enter", { worldId, server: status.server }).then(
      (fresh) => {
        setNotice(t("roomGoneNotice", { code: fresh.roomCode ?? "" }));
        void connect(false);
      },
      () => setFailed(true),
    );
  }, [status, worldId, connect]);

  // 自動入座：保留給電腦的那張卡還空著就認領
  const view = room?.view ?? null;
  const act = room?.act;
  useEffect(() => {
    if (!view || !act || seatTried.current || view.me.cardId || !mine) return;
    const card = view.cards.find((c) => c.cardId === mine);
    if (!card || card.claimedBy !== null) return;
    seatTried.current = true;
    void act("seat.claim", { cardId: mine });
  }, [view, act, mine]);

  const { onPaused } = deps;
  const pause = useCallback(async () => {
    const ok = await confirm(t("roomPauseConfirm"), { title: t("roomPause"), kind: "warning", okLabel: t("roomPause"), cancelLabel: t("dialogCancel") });
    if (!ok) return;
    setBusy(true);
    try {
      await invoke("room_exit");
      onPaused();
    } finally {
      setBusy(false);
    }
  }, [onPaused]);

  async function toPhone() {
    const ok = await confirm(t("roomToPhoneConfirm"), { title: t("roomToPhone"), kind: "warning", okLabel: t("roomToPhone"), cancelLabel: t("dialogCancel") });
    if (!ok) return;
    setBusy(true);
    setHandoff(await freshPairLink(true));
    setBusy(false);
  }

  async function reconnect() {
    if (!worldId) return;
    await invoke("room_enter", { worldId, server: status?.server ?? "" }).then(() => connect(false), () => setFailed(true));
  }

  const stopped = failed || (status !== null && !status.active && !status.gone);
  // 連線本體要開著的那一份（RoomSession 掛在 app 最外層）
  const live = worldId && join && !stopped ? join : null;

  return {
    worldId,
    status,
    room,
    view,
    /** 玩家卡區該放的卡：電腦坐的那張，還沒坐就是保留給它的那張；還沒連上＝undefined（照桌上的設定） */
    seat: view ? (view.me.cardId ?? mine) : undefined,
    notice,
    handoff,
    nickname,
    setNickname,
    busy,
    stopped,
    /** 房間伺服器來源（房主面板重生邀請連結用） */
    origin: join?.origin ?? "",
    live,
    setRoom,
    takeHost: () => void connect(true),
    pause: () => void pause(),
    toPhone: () => void toPhone(),
    reconnect: () => void reconnect(),
  };
}
