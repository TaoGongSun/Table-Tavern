// 房間連線＋匯出＋上傳：手機外殼（App.tsx）與桌面版（src/features/room/）共用。
import { rememberJoined, withoutPairCode, type Invite } from "../connection/invite";
import { makeUpload } from "../my-card/card-upload";
import { useRoom, type RoomEnv } from "./useRoom";

/** 房間連線＋匯出：桌面版放在 app 最外層，離開房間畫面（編輯卡、回大廳）連線也不斷 */
export function useRoomSession(env: RoomEnv, invite: Invite, device: string, nickname: string) {
  const room = useRoom(env, { code: invite.code, deviceId: device, joinSecret: invite.joinSecret, pairCode: invite.pairCode, nickname }, () => {
    rememberJoined(env.storage, invite.code, nickname);
    env.replaceHref(withoutPairCode(env.href));
  });
  const exportRoom = async () => {
    try {
      const res = await env.fetch(`/api/rooms/${invite.code}/export`, { headers: { Authorization: `Bearer ${device}` } });
      if (!res.ok) return false;
      const a = document.createElement("a");
      a.href = URL.createObjectURL(await res.blob());
      a.download = `table-tavern-room-${invite.code}.json`;
      a.click();
      URL.revokeObjectURL(a.href);
      return true;
    } catch {
      return false;
    }
  };
  const upload = makeUpload(env.fetch, invite.code, device, room.awaitUpload);
  return { ...room, exportRoom, upload, joinSecret: invite.joinSecret };
}
export type RoomSession = ReturnType<typeof useRoomSession>;
