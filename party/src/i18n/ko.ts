// 未翻譯的文案沿用英文；room-desktop-layout 的名單與接回提示已翻譯。
import { en } from "./en";
import type { PartyMessages } from "./zh-TW";

export const ko: PartyMessages = {
  ...en,
  statusTime: "시간",
  statusPlace: "장소",
  statusPresent: "등장",
  hostRemove: "제거…",
  hostKick: "제거",
  seatRelease: "자리 양보",
  seatReleaseConfirm:
    "자리를 양보할까요? 이 캐릭터를 더 이상 조종하지 않습니다. 다시 앉으려면 캐릭터를 새로 선택해야 합니다.",
  badgePayer: "이 사람의 한도 사용",
  badgePayerSelf: "내 한도 사용",
  badgePayerHint: "GM은 이 사람의 AI 사용 한도로 실행됩니다",
  pairTitle: "내 한도로 GM 실행: 컴퓨터에 표시된 페어링 코드 입력",
  roundAllDelegatedHost: "모두 AI 대행 중: GM 진행을 누르거나 누군가의 대행을 해제하세요",
  delegatedReturned: "자리를 비운 동안의 이야기는 위에 있습니다. 위로 스크롤해서 읽어 보세요.",
  hostMemberOptions: "{name} 관리",
};
