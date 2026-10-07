// 內建範例卡：一張新寫的單角色卡（D5），ST chara_card_v3 格式，完整外殼照卡片契約保留。各語系一份（samples/），
// 開始畫面照目前語系給；開了桌之後卡就是那一份，換語系不換卡。
import type { Lang } from "@desktop/i18n/languages";
import { playCardFromValue, type PlayCard } from "./play-card";
import { de } from "./samples/de";
import { en } from "./samples/en";
import { es } from "./samples/es";
import { fr } from "./samples/fr";
import { ja } from "./samples/ja";
import { ko } from "./samples/ko";
import { ptBR } from "./samples/pt-BR";
import { ru } from "./samples/ru";
import type { SampleText } from "./samples/sample-text";
import { zhCN } from "./samples/zh-CN";
import { zhTW } from "./samples/zh-TW";

export interface CharacterCardData {
  name: string;
  description: string;
  personality: string;
  scenario: string;
  first_mes: string;
  mes_example: string;
  creator_notes: string;
  system_prompt: string;
  post_history_instructions: string;
  alternate_greetings: string[];
  tags: string[];
  creator: string;
  character_version: string;
  extensions: Record<string, unknown>;
}

export interface CharacterCard {
  spec: "chara_card_v3";
  spec_version: "3.0";
  data: CharacterCardData;
}

export const SAMPLE_TEXTS: Record<Lang, SampleText> = {
  "zh-TW": zhTW,
  "zh-CN": zhCN,
  en,
  ja,
  ko,
  es,
  "pt-BR": ptBR,
  de,
  fr,
  ru,
};

export function sampleCard(lang: Lang): CharacterCard {
  const text = SAMPLE_TEXTS[lang];
  return {
    spec: "chara_card_v3",
    spec_version: "3.0",
    data: {
      ...text,
      tags: [...text.tags],
      mes_example: "",
      system_prompt: "",
      post_history_instructions: "",
      alternate_greetings: [],
      creator: "Table Tavern",
      character_version: "1.0",
      extensions: {},
    },
  };
}

const playCards = new Map<Lang, PlayCard>();

/** 開始畫面那張範例卡（同一語系同一份，重繪不重解） */
export function samplePlayCard(lang: Lang): PlayCard {
  let card = playCards.get(lang);
  if (!card) {
    card = playCardFromValue("builtin", sampleCard(lang));
    playCards.set(lang, card);
  }
  return card;
}

/** 繁中那一份（測試與既有呼叫端用） */
export const SAMPLE_CARD = sampleCard("zh-TW");
export const SAMPLE_PLAY_CARD = samplePlayCard("zh-TW");
