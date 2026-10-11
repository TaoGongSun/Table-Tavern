// 卡片介面殼的選路：這桌現在該顯示哪一份殼、它是從哪一樓產生的，以及交給卡片的每一樓文字。
// 純函式，controller 只負責接線。
import { cardDrawable, cardKey, findShell, gmOrder, type CardInterface, type ShellCache, type ShellCandidate } from "./interface-card";
import { type ChatFloor, type ChatRole, type CurrentFloor } from "./card-chat-shim";
import { buildCardMvu, hasStatData, withMvuPlaceholder, type CardMvu, type MvuLayer, type StateTree } from "./mvu/card-mvu-shim";
import { fillSkeletonPlaceholders, type StateNode } from "../refactor/refactor-shell";
import { type TranscriptEvent } from "../../shared/contracts/backend-contracts";
import { eventDisplayText, parseMarker, speakerDisplayName } from "../../shared/ui/event-text";

/**
 * 本場的「樓」：gm_only 的系統事件（角色私設、非公開人物全文）不算——酒館的聊天紀錄裡沒有這類
 * app 內部資料，玩家面的面板也不該拿到。回傳每樓對應的事件，樓號＝陣列位置。
 */
export function chatEvents(events: TranscriptEvent[]): TranscriptEvent[] {
  return events.filter((event) => !event.gm_only);
}

/** 酒館存的是顯示 regex 套用前的原文；帶標頭代碼的系統事件給照語系組好的全文 */
export function floorText(event: TranscriptEvent): string {
  return event.raw ?? eventDisplayText(event);
}

export interface PickedShell {
  shell: string;
  /** 產生這份殼的卡（cardKey）：分頁鈕亮哪顆、MVU character 層身分 */
  card: string;
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

interface FloorBuild {
  message: string;
  /** 候選來源：原文的角色台詞樓帶說話者；對得上卡片開場白的開場樓帶所屬卡；其餘比照 GM 樓試卡 */
  speaker?: string;
  owner?: string;
  /** 骨架合成樓：只歸骨架來源卡 */
  synthesized?: boolean;
  /** 前情提要樓：message 的正文槽放的是 slot，選完殼才換回 text */
  recap?: { slot: string; text: string };
  /** 不參與選殼（缺快照的前情提要樓） */
  noShell?: boolean;
}

// 正文槽的前情提要副本〔作者裁決 2026-10-06〕：正文槽不渲染 Markdown，只拿掉成對強調（**…**、__…__）與
// 獨立成行的分隔線（---、***、___、——），條列照留；單獨的 *、算式、行內 --- 不動。儲存原文與聊天欄不受影響。
const PAIRED_EMPHASIS = /(\*\*|__)(?=\S)(.+?)(?<=\S)\1/g;
// 行尾容許 \r：CRLF 文字照行切開後每行帶著 \r
const RULE_LINE = /^[ \t]*(?:([-*_])(?:[ \t]*\1){2,}|[—─]+)[ \t\r]*$/;

function plainRecap(text: string): string {
  return text
    .split("\n")
    .filter((line) => !RULE_LINE.test(line))
    .map((line) => line.replace(PAIRED_EMPHASIS, "$2"))
    .join("\n");
}

// 前情提要換回正文槽時把角括號轉全形：殼內腳本與卡片讀訊息時都不會把摘要裡的字當標籤
function inertRecap(text: string): string {
  return plainRecap(text).replace(/</g, "＜").replace(/>/g, "＞");
}

// 前情提要佔位：英數字（卡 regex 最不會動它），帶這次選路才產生的亂數；亂數保證不出現在骨架、
// 逐字稿與狀態樹任何地方，換回時就不會誤換到既有資料（面板值、別樓正文）
const SLOT_PREFIX = "TTRECAP";

function randomNonce(): string {
  const bytes = new Uint8Array(8);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function pickNonce(haystack: string, next: () => string): string {
  for (;;) {
    const nonce = next();
    if (/^[A-Za-z0-9]+$/.test(nonce) && !haystack.includes(`${SLOT_PREFIX}${nonce}`)) return nonce;
  }
}

/** 角色台詞樓的說話者：候選來源用它決定只給說話者自己的卡（有的話） */
function speakerOf(event: TranscriptEvent): string | undefined {
  return event.kind === "dialogue" && event.speaker_id ? event.speaker_id : undefined;
}

/**
 * 已貼出的開場白：原文（或正文）去頭尾空白後等於某張卡的開場白（多張相同取第一張），而且那張卡畫得出東西，就認那張卡；
 * 對不上（巨集已代換、選了別的 alternate_greetings 等）或那張卡沒介面，照 GM 樓規則（同說話者規則）
 */
function openingOwner(event: TranscriptEvent, cards: CardInterface[]): string | undefined {
  if (event.opening !== true) return undefined;
  const texts = [event.raw, event.text].filter((text): text is string => typeof text === "string").map((text) => text.trim());
  const card = cards.find((card) => {
    const opening = card.opening?.trim();
    return opening !== undefined && opening !== "" && texts.includes(opening);
  });
  return card === undefined || !cardDrawable(card) ? undefined : cardKey(card);
}

/**
 * 一樓交給卡片的文字。對照酒館：狀態欄跟著每則訊息走，每樓帶自己那一刻的狀態區塊。
 * 有重構骨架時，GM 旁白／角色對話這兩種樓依序：原文自己畫得出殼（重構前的開場白、舊回合）就用原文；
 * 否則用這一樓存下的狀態快照（state.tree）加這一樓的正文填骨架；沒有快照就用原文。
 * 「原文畫得出殼」與選殼同一套候選規則（說話者優先、按卡隔離），指定卡時只看那張卡：指定的若是骨架來源卡，
 * 別張卡畫得出的原文樓在這次選路裡改填骨架。合成樓（synthesized）只歸骨架來源卡（pickCardShell 決定）。
 * 前情提要樓（換幕摘要）例外：不走原文、一律填骨架，正文槽先放佔位，選殼的 regex 與圍欄抽取都碰不到
 * 摘要本文，選完才換回；沒有快照就用目前檯面樹（摘要在幕首，檯面樹就是它那一刻的值），連檯面樹都沒有
 * 就不參與選殼。玩家與 system 樓一律原文。合成只在記憶體裡，不寫回逐字稿。
 */
function floorMessage(
  event: TranscriptEvent,
  skeleton: string | null,
  cards: CardInterface[],
  yamlTags: string[],
  valueTypes: Record<string, string>,
  liveTree: StateTree | undefined,
  slot: string,
  pick: { only: string | null; cache?: ShellCache },
): FloorBuild {
  const raw = floorText(event);
  const source = { speaker: speakerOf(event), owner: openingOwner(event, cards) };
  if (skeleton === null || (event.kind !== "narration" && event.kind !== "dialogue")) {
    return { message: raw, ...source };
  }
  const snapshot = event.state?.tree as Record<string, StateNode> | undefined;
  if (parseMarker(event.marker)?.type === "scene_summary") {
    const tree = snapshot ?? liveTree;
    if (tree === undefined) return { message: raw, noShell: true };
    return {
      message: fillSkeletonPlaceholders(skeleton, { ...tree, 本回合: { 正文: slot } }, yamlTags, valueTypes),
      recap: { slot, text: inertRecap(eventDisplayText(event)) },
      synthesized: true,
    };
  }
  if (snapshot === undefined || findShell(cards, [{ text: raw, ...source }], pick) !== null) return { message: raw, ...source };
  return {
    message: fillSkeletonPlaceholders(skeleton, { ...snapshot, 本回合: { 正文: eventDisplayText(event) } }, yamlTags, valueTypes),
    synthesized: true,
  };
}

export interface PickInput {
  /** 桌面玩法標記；undefined＝還不知道 */
  tableMode: string | null | undefined;
  /** AI 重構產的介面骨架（interface-shell.html）；null＝沒有 */
  refactorShell: string | null;
  events: TranscriptEvent[];
  cardInterfaces: CardInterface[];
  /** 原卡欄位型別（mechanism.value_types）：決定骨架裡數字／布林要不要加引號 */
  valueTypes?: Record<string, string>;
  /** 前情提要佔位的亂數來源（測試注入用；預設密碼學亂數） */
  nonce?: () => string;
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
  /** 指定卡（cardKey）：只用那張卡、掃整幕找它最新畫得出的一樓；null／不給＝跟著最新 */
  card?: string | null;
  /** 卡套文字的結果快取（controller 持有，卡清單一換就換新） */
  cache?: ShellCache;
}

/** 跟著最新時往回掃幾樓（最新 GM 樓另外先試、不受此限）；指定卡掃整幕 */
const RECENT_FLOORS = 10;

export function pickCardShell(input: PickInput): PickedShell | null {
  const { tableMode, cardInterfaces } = input;
  const only = input.card ?? null;
  const pickOptions = { only, cache: input.cache };
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
  // 補好的文字選殼與讀訊息共用同一份。{{char}} 先用第一張 MVU 卡的名字補占位，命中別張 MVU 卡時換成它的
  const mvuCard = cardInterfaces.find((card) => card.mvu === true && card.unsupported === null);
  const buildMvu = (char: string | null) =>
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
          macros: { user: input.mvu.userName, char },
        })
      : null;
  const mvu = buildMvu(mvuCard?.character_name || null);
  const nonce =
    skeleton === null
      ? ""
      : pickNonce(
          skeleton + JSON.stringify(input.events) + JSON.stringify(input.mvu?.liveTree ?? null),
          input.nonce ?? randomNonce,
        );
  const builds = events.map((event, id) =>
    floorMessage(
      event,
      skeleton,
      cardInterfaces,
      yamlTags,
      input.valueTypes ?? {},
      input.mvu?.liveTree,
      `${SLOT_PREFIX}${nonce}N${id}E`,
      pickOptions,
    ),
  );
  const floors = events.map((event, id) => {
    const { message, noShell, speaker, owner } = builds[id];
    return {
      event,
      id,
      speaker,
      owner,
      noShell: noShell === true,
      message:
        mvu === null
          ? message
          : withMvuPlaceholder(message, event.opening === true, roleOf(event), hasStatData(mvu, id)),
    };
  });
  // 前情提要的佔位換回本文：選殼與抽殼都已完成，之後只剩顯示。一次掃完，換進去的本文不再被後續替換
  const recaps = new Map(builds.flatMap(({ recap }) => (recap === undefined ? [] : [[recap.slot, recap.text] as const])));
  const slotPattern = recaps.size === 0 ? null : new RegExp(`${SLOT_PREFIX}${nonce}N\\d+E`, "g");
  const restore = (text: string): string =>
    slotPattern === null ? text : text.replace(slotPattern, (slot) => recaps.get(slot) ?? slot);
  // 先試最新一個 GM 樓（旁白／角色對話）：後面接再多玩家樓，本樓都不會被擠出視窗；再往前掃非玩家樓
  // （跟著最新掃近 10 樓、指定卡掃整幕）；空桌退回卡片開場白——這類卡的開場就是一整頁選角畫面，各自只給所屬的卡
  const latestGm = [...floors]
    .reverse()
    .find(({ event, noShell }) => !noShell && (event.kind === "narration" || event.kind === "dialogue"));
  const recent = [
    ...(latestGm ? [latestGm] : []),
    ...(only === null ? floors.slice(-RECENT_FLOORS) : floors)
      .filter(({ event, noShell }) => !noShell && event.kind !== "player")
      .reverse()
      .filter((floor) => floor !== latestGm),
  ];
  // 骨架來源卡：最新一個合成樓照 GM 樓順序第一張畫得出它的卡（骨架照搬的是那張卡的輸出格式）。
  // 合成樓只歸它：同標籤的別張卡被指定時畫不出骨架
  let skeletonOwner: string | undefined;
  for (const floor of [...floors].reverse()) {
    const build = builds[floor.id];
    if (!build.synthesized || floor.noShell) continue;
    const hit = findShell(cardInterfaces, [{ text: floor.message, noCache: build.recap !== undefined }], { cache: input.cache });
    if (hit !== null) {
      skeletonOwner = cardKey(hit.card);
      break;
    }
  }
  const openings = floors.length === 0 ? gmOrder(cardInterfaces) : [];
  const candidates: ShellCandidate[] = [
    ...recent.map(({ id, message, speaker, owner }) =>
      builds[id].synthesized
        ? { text: message, owner: skeletonOwner ?? "", noCache: builds[id].recap !== undefined }
        : { text: message, speaker, owner },
    ),
    ...openings.map((card) => ({ text: card.opening, owner: cardKey(card) })),
  ];
  // 前情提要樓選出的殼必須還帶著完整佔位：卡腳本若把佔位改掉或吃掉，換不回本文，這份殼就會漏掉摘要。
  // 這種樓不出殼，改試下一個候選（較舊的樓或開場白），都沒有就不顯示介面，絕不交付漏正文的殼
  const skipped = new Set<number>();
  let match: ReturnType<typeof findShell> = null;
  for (;;) {
    match = findShell(
      cardInterfaces,
      candidates.map((candidate, index) => (skipped.has(index) ? null : candidate)),
      pickOptions,
    );
    if (match === null) return null;
    const slot = match.index < recent.length ? builds[recent[match.index].id].recap?.slot : undefined;
    if (slot === undefined || match.shell.includes(slot)) break;
    skipped.add(match.index);
  }
  const hit = match.card;
  const card = cardKey(hit);
  // MVU character 層身分＝產生這份殼的卡；命中的是另一張 MVU 卡時 {{char}} 換成它的名字
  const owned = hit.mvu === true && hit !== mvuCard ? buildMvu(hit.character_name || null) : mvu;
  const withOwner = (value: CardMvu | null, currentId: number): CardMvu | null =>
    value === null ? null : { ...value, currentId, characterId: card };
  const chatFloors: ChatFloor[] = floors.map(({ event, message }) => ({
    name: speakerDisplayName(event),
    role: roleOf(event),
    message: restore(message),
  }));
  if (match.index < recent.length) {
    const { event, id, message } = recent[match.index];
    return {
      shell: restore(match.shell),
      card,
      current: { id, name: speakerDisplayName(event), text: restore(message) },
      floors: chatFloors,
      mvu: withOwner(owned, id),
    };
  }
  return {
    shell: match.shell,
    card,
    current: { id: 0, name: hit.character_name, text: hit.opening ?? "" },
    floors: chatFloors,
    mvu: withOwner(owned, 0),
  };
}

/**
 * 分頁鈕要列的卡：桌上畫得出東西的卡不到兩張就不列（單卡桌沒有分頁）。直接拿「指定卡選殼」逐張跑，
 * 與指定卡同一套候選規則、掃整幕，分頁才不會因為最近幾樓換人說話而忽隱忽現。順序：世界書卡先、其餘照桌上順序。
 */
export function drawableCards(input: PickInput): { key: string; name: string }[] {
  const usable = input.cardInterfaces.filter(cardDrawable);
  if (usable.length < 2) return [];
  return gmOrder(usable)
    .filter((card) => pickCardShell({ ...input, card: cardKey(card) }) !== null)
    .map((card) => ({ key: cardKey(card), name: card.character_name }));
}
