// 卡片介面殼的選路：這桌現在該顯示哪一份殼、它是從哪一樓產生的，以及交給卡片的每一樓文字。
// 純函式，controller 只負責接線。
import { findShell, type CardInterface } from "./interface-card";
import { chatEvents, floorText, type ChatFloor, type ChatRole, type CurrentFloor } from "./card-chat-shim";
import { buildCardMvu, hasStatData, withMvuPlaceholder, type CardMvu, type MvuLayer, type StateTree } from "./mvu/card-mvu-shim";
import { fillSkeletonPlaceholders, type StateNode } from "../refactor/refactor-shell";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { eventDisplayText, speakerDisplayName } from "../../shared/ui/event-text";

export interface PickedShell {
  shell: string;
  /** 產生這份殼的那一樓；讀訊息墊片的 getCurrentMessageId 回它 */
  current: CurrentFloor;
  /** 交給卡片的每一樓（樓號＝位置）；本樓的文字與 current.text 是同一份 */
  floors: ChatFloor[];
  /** MVU 變數快照；null＝這桌沒有載入 MVU 的卡（或走重構骨架），沙盒不定義 MVU 函式 */
  mvu: CardMvu | null;
}

// 卡的顯示腳本自己用 YAML 解析器讀狀態區塊時，那支腳本抓的容器（例如 <Status_block>）裡的值要照 YAML
// 語法寫，卡讀到的才是原值；容器外（正文槽、其他格式）與別支腳本照原樣填
const YAML_PARSER = /js-?yaml|jsyaml|yaml\.load|YAML\.parse/i;
const REGEX_TAG = /<\\?\/?([A-Za-z_][\w.-]*)/g;

export function skeletonYamlTags(cards: CardInterface[]): string[] {
  const tags = new Set<string>();
  for (const card of cards) {
    for (const script of card.scripts) {
      if (!YAML_PARSER.test(script.replace_string)) continue;
      for (const match of script.find_regex.matchAll(REGEX_TAG)) tags.add(match[1]);
    }
  }
  return [...tags];
}

function roleOf(event: TranscriptEvent): ChatRole {
  if (event.kind === "player") return "user";
  if (event.kind === "system") return "system";
  return "assistant";
}

/**
 * 一樓交給卡片的文字。對照酒館：狀態欄跟著每則訊息走，每樓帶自己那一刻的狀態區塊。
 * 有重構骨架時，GM 旁白／角色對話這兩種樓依序：原文自己畫得出殼（重構前的開場白、舊回合）就用原文；
 * 否則用這一樓存下的狀態快照（state.tree）加這一樓的正文填骨架；沒有快照就用原文。
 * 玩家與 system 樓一律原文。合成只在記憶體裡，不寫回逐字稿。
 */
function floorMessage(
  event: TranscriptEvent,
  skeleton: string | null,
  cards: CardInterface[],
  yamlTags: string[],
  valueTypes: Record<string, string>,
): string {
  const raw = floorText(event);
  if (skeleton === null || (event.kind !== "narration" && event.kind !== "dialogue")) return raw;
  const tree = event.state?.tree as Record<string, StateNode> | undefined;
  if (tree === undefined || findShell(cards, [raw]) !== null) return raw;
  return fillSkeletonPlaceholders(skeleton, { ...tree, 本回合: { 正文: eventDisplayText(event) } }, yamlTags, valueTypes);
}

/**
 * 實際產生這份殼的卡（`character` 層變數的身分，由宿主決定、不信沙盒）：只看選殼實際命中的那一段候選文字
 * （身分跟著選殼結果走，不重掃其他候選），只用單張卡自己的腳本就能從它產出同一份殼的第一張卡；世界書卡（沒有 id）固定 `world`。多張卡的腳本接力才產得出來、或找不到，回空字串
 * （沙盒讀寫 character 層一律拋錯、宿主也拒絕），不猜。
 */
export function shellOwner(cards: CardInterface[], text: string | null | undefined, shell: string): string {
  for (const card of cards) {
    if (card.unsupported !== null) continue;
    if (findShell([card], [text])?.shell === shell) return card.character_id || "world";
  }
  return "";
}

export function pickCardShell(input: {
  /** 桌面玩法標記；undefined＝還不知道 */
  tableMode: string | null | undefined;
  /** AI 重構產的介面骨架（interface-shell.html）；null＝沒有 */
  refactorShell: string | null;
  events: TranscriptEvent[];
  cardInterfaces: CardInterface[];
  /** 原卡欄位型別（mechanism.value_types）：決定骨架裡數字／布林要不要加引號 */
  valueTypes?: Record<string, string>;
  /** 目前狀態樹（含面板手動改值）與玩家名：MVU 卡的活樓讀它；active＝卡片變數模式（每樓讀事件上自己的表）；
   *  null＝不給 MVU 快照 */
  mvu?: {
    liveTree: StateTree;
    userName: string;
    active?: boolean;
    generation?: number;
    scene?: number;
    /** 非 message 層現況（面板掛載時從後端讀回） */
    layers?: Record<string, MvuLayer>;
  } | null;
}): PickedShell | null {
  const { tableMode, cardInterfaces } = input;
  // 角色優先桌：介面產物一律不建不顯示（refactor-mode-split 拍板）——重構骨架、卡片自帶殼、
  // 掃 raw 的 fallback 整組短路。標記還沒讀回（undefined）也先不顯示，未知就放行會在角色桌
  // 切桌瞬間閃出介面。
  if (tableMode === undefined || tableMode === "characters") return null;
  const skeleton =
    input.refactorShell !== null && input.refactorShell.trim() !== "" ? input.refactorShell : null;
  const yamlTags = skeletonYamlTags(cardInterfaces);
  // 樓號＝本場「樓」的位置（gm_only 事件不算一樓）；每樓的文字只算這一次，選殼、本樓、歷史樓、
  // 掛載與推送都用同一份。
  const events = chatEvents(input.events);
  // MVU 卡（沒重構、照原卡畫面）：每樓的變數表，與公開樓號同一套索引；有 stat_data 的非玩家樓比照 MVU 補占位，
  // 補好的文字選殼與讀訊息共用同一份
  const mvuCard = cardInterfaces.find((card) => card.mvu === true && card.unsupported === null);
  const mvu =
    skeleton === null && mvuCard !== undefined && input.mvu
      ? buildCardMvu({
          events,
          positions: events.map((event) => input.events.indexOf(event)),
          active: input.mvu.active === true,
          generation: input.mvu.generation,
          scene: input.mvu.scene,
          layers: input.mvu.layers,
          roles: events.length > 0 ? events.map(roleOf) : ["assistant"],
          currentId: 0,
          liveTree: input.mvu.liveTree,
          valueTypes: input.valueTypes ?? {},
          macros: { user: input.mvu.userName, char: mvuCard.character_name || null },
        })
      : null;
  const floors = events.map((event, id) => {
    const message = floorMessage(event, skeleton, cardInterfaces, yamlTags, input.valueTypes ?? {});
    return {
      event,
      id,
      message:
        mvu === null
          ? message
          : withMvuPlaceholder(message, event.opening === true, roleOf(event), hasStatData(mvu, id)),
    };
  });
  // 先試最新一個 GM 樓（旁白／角色對話）：後面接再多玩家樓，本樓都不會被擠出視窗；再往前掃最近 10 樓的
  // 非玩家樓；空桌退回卡片開場白——這類卡的開場就是一整頁選角畫面
  const latestGm = [...floors]
    .reverse()
    .find(({ event }) => event.kind === "narration" || event.kind === "dialogue");
  const recent = [
    ...(latestGm ? [latestGm] : []),
    ...floors
      .slice(-10)
      .filter(({ event }) => event.kind !== "player")
      .reverse()
      .filter((floor) => floor !== latestGm),
  ];
  const openings = floors.length === 0 ? cardInterfaces : [];
  const candidates = [...recent.map(({ message }) => message), ...openings.map((card) => card.opening)];
  const match = findShell(cardInterfaces, candidates);
  if (match === null) return null;
  const characterId = shellOwner(cardInterfaces, candidates[match.index], match.shell);
  const withOwner = (value: CardMvu | null): CardMvu | null => (value === null ? null : { ...value, characterId });
  const chatFloors: ChatFloor[] = floors.map(({ event, message }) => ({
    name: speakerDisplayName(event),
    role: roleOf(event),
    message,
  }));
  if (match.index < recent.length) {
    const { event, id, message } = recent[match.index];
    return {
      shell: match.shell,
      current: { id, name: speakerDisplayName(event), text: message },
      floors: chatFloors,
      mvu: withOwner(mvu === null ? null : { ...mvu, currentId: id }),
    };
  }
  const card = openings[match.index - recent.length];
  return {
    shell: match.shell,
    current: { id: 0, name: card.character_name, text: card.opening ?? "" },
    floors: chatFloors,
    mvu: withOwner(mvu),
  };
}
