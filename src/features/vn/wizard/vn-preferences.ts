// VN 精靈 ① 的選項表（plans/vn-setup-wizard.md §2 ①）：題材（戀愛排第一，只在 VN 桌用）、氛圍、長度、選擇。
import { t, type MsgKey } from "../../../i18n";
import type { VnPreferences } from "../../../shared/contracts/backend-contracts";

export const VN_GENRES = [
  "vnGenreRomance",
  "vnGenreMystery",
  "vnGenreHorror",
  "vnGenreFantasy",
  "vnGenreScifi",
  "vnGenreSchool",
  "vnGenreSliceOfLife",
  "vnGenreHistorical",
  "vnGenreApocalypse",
  "vnGenreWorkplace",
] as const satisfies readonly MsgKey[];

export const MOODS = [
  ["light", "vnMoodLight"],
  ["warm", "vnMoodWarm"],
  ["dark", "vnMoodDark"],
] as const satisfies readonly (readonly [NonNullable<VnPreferences["mood"]>, MsgKey])[];

export const LENGTHS = [
  ["short", "vnLengthShort"],
  ["medium", "vnLengthMedium"],
  ["long", "vnLengthLong"],
] as const satisfies readonly (readonly [NonNullable<VnPreferences["length"]>, MsgKey])[];

export const CHOICES = [
  ["none", "vnChoiceNone"],
  ["light", "vnChoiceLight"],
  ["full", "vnChoiceFull"],
] as const satisfies readonly (readonly [NonNullable<VnPreferences["branching"]>, MsgKey])[];

/** ① 的表單狀態：題材存鍵，送後端時換成介面語言文字。主角一句話在大綱上，不在這張表 */
export interface PreferenceDraft extends Omit<VnPreferences, "genres" | "tagline"> {
  genres: (typeof VN_GENRES)[number][];
}

export const EMPTY_DRAFT: PreferenceDraft = { genres: [], mood: null, length: null, branching: null, name: "" };

export const genreLabels = (draft: PreferenceDraft) => draft.genres.map((key) => t(key));

/** `tagline` 只在建桌時帶（大綱的主角一句話）。生成大綱時還沒有，留空 */
export const toPreferences = (draft: PreferenceDraft, tagline = ""): VnPreferences => ({
  ...draft,
  genres: genreLabels(draft),
  name: draft.name.trim(),
  tagline: tagline.trim(),
});

/** 一句話的範例句：依第一個勾的題材換，沒勾用中性例子 */
export function ideaPlaceholder(draft: PreferenceDraft): string {
  const first = draft.genres[0];
  return t(first ? (`${first}Example` as MsgKey) : "vnIdeaExample");
}
