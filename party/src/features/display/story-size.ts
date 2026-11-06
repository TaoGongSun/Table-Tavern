// 劇情字級（玩家自調，只影響自己這支手機）：四段，存在瀏覽器；App.css 用 --story-size 套到劇情段落與輸入框。
export const STORY_SIZES = { s: 15, m: 17, l: 19, xl: 22 } as const;
export type StorySize = keyof typeof STORY_SIZES;

const KEY = "tt-party:story-size";

export function loadStorySize(storage: Storage | null): StorySize {
  try {
    const saved = storage?.getItem(KEY);
    return saved && saved in STORY_SIZES ? (saved as StorySize) : "m";
  } catch {
    return "m";
  }
}

export function saveStorySize(storage: Storage | null, size: StorySize) {
  try {
    storage?.setItem(KEY, size);
  } catch {
    // 瀏覽器不讓存：這次照樣生效，下次回預設
  }
}
