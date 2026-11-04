// 未翻譯的文案沿用英文；room-desktop-layout 的名單與接回提示已翻譯。
import { en } from "./en";
import type { PartyMessages } from "./zh-TW";

export const ptBR: PartyMessages = {
  ...en,
  guideLink: "Como jogar",
  statusTime: "Tempo",
  statusPlace: "Local",
  statusPresent: "Presentes",
  hostRemove: "Remover…",
  hostKick: "Remover",
  seatRelease: "Ceder o lugar",
  seatReleaseConfirm:
    "Ceder seu lugar? Você deixará de controlar este personagem e precisará escolher um para se sentar de novo.",
  badgePayer: "Usando a cota desta pessoa",
  badgePayerSelf: "Usando minha cota",
  badgePayerHint: "O GM usa a cota de IA desta pessoa",
  pairTitle: "Usar minha cota para o GM: digite o código de pareamento do computador",
  roundAllDelegatedHost: "Todos com IA: avance com o GM ou desative a IA de alguém",
  delegatedReturned: "A história de enquanto você esteve fora está acima. Role para cima para ler.",
  hostMemberOptions: "Gerenciar {name}",
};
