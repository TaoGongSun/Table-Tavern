// 未翻譯的文案沿用英文；room-desktop-layout 的名單與接回提示已翻譯。
import { en } from "./en";
import type { PartyMessages } from "./zh-TW";

export const ru: PartyMessages = {
  ...en,
  guideLink: "Как играть",
  statusTime: "Время",
  statusPlace: "Место",
  statusPresent: "Присутствуют",
  hostRemove: "Удалить…",
  hostKick: "Удалить",
  seatRelease: "Освободить место",
  seatReleaseConfirm:
    "Освободить место? Вы перестанете управлять этим персонажем. Чтобы снова сесть, потребуется выбрать персонажа.",
  badgePayer: "Использует его квоту",
  badgePayerSelf: "Использует мою квоту",
  badgePayerHint: "GM работает за счёт квоты ИИ этого участника",
  pairTitle: "Использовать мою квоту для GM: введите код сопряжения с компьютера",
  roundAllDelegatedHost:
    "Все под управлением ИИ: продвиньте сюжет через GM или отключите кому-нибудь ИИ",
  delegatedReturned:
    "Сюжет за время вашего отсутствия находится выше. Прокрутите вверх, чтобы прочитать.",
  hostMemberOptions: "Управление: {name}",
  storyJumpLatest: "↓ К последнему",
  displayTitle: "Вид",
  roomCode: "Комната {code}",
  storySize: "Размер текста",
  storySize_s: "S",
  storySize_m: "M",
  storySize_l: "L",
  storySize_xl: "XL",
  pairFold: "Запустить GM на моей квоте",
  actionFold: "▾ Свернуть",
  actionWrite: "✎ Написать действие",
  actionDraft: "✎ Черновик",
};
