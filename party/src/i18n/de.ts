// 未翻譯的文案沿用英文；room-desktop-layout 的名單與接回提示已翻譯。
import { en } from "./en";
import type { PartyMessages } from "./zh-TW";

export const de: PartyMessages = {
  ...en,
  guideLink: "So wird gespielt",
  statusPanel: "Statusübersicht",
  statusClose: "Schließen",
  statusItems: "Gegenstände",
  statusTracking: "Verfolgung",
  statusGoals: "Ziele",
  statusPromises: "Versprechen",
  statusClues: "Hinweise",
  statusTime: "Zeit",
  statusPlace: "Ort",
  statusPresent: "Anwesend",
  hostRemove: "Entfernen…",
  hostKick: "Entfernen",
  seatRelease: "Platz freigeben",
  seatReleaseConfirm:
    "Deinen Platz freigeben? Du steuerst diese Figur dann nicht mehr und musst zum erneuten Platznehmen eine Figur auswählen.",
  badgePayer: "Nutzt deren Kontingent",
  badgePayerSelf: "Nutzt mein Kontingent",
  badgePayerHint: "Der GM läuft mit dem KI-Kontingent dieser Person",
  pairTitle: "Mein Kontingent für den GM nutzen: Kopplungscode vom Computer eingeben",
  roundAllDelegatedHost: "Alle mit KI: GM fortsetzen oder die KI für jemanden ausschalten",
  delegatedReturned:
    "Die Geschichte während deiner Abwesenheit steht weiter oben. Scrolle hoch, um sie zu lesen.",
  hostMemberOptions: "{name} verwalten",
  storyJumpLatest: "↓ Zurück zum Neuesten",
  displayTitle: "Anzeige",
  roomCode: "Raum {code}",
  storySize: "Schriftgröße der Geschichte",
  storySize_s: "S",
  storySize_m: "M",
  storySize_l: "L",
  storySize_xl: "XL",
  pairFold: "GM mit meinem Kontingent",
  actionFold: "▾ Einklappen",
  actionWrite: "✎ Aktion schreiben",
  actionDraft: "✎ Entwurf",
};
