// UI 語系字典：zh-TW 為正典，en 逐鍵對應（缺鍵時 TypeScript 直接報錯）。
// 語系存 config.preferences.language；App 每次 render 前呼叫 setLang 同步，
// 元件一律經 t() 取字串。模型輸出語言規範在後端 transport.rs，不在這裡。

export type Lang = "zh-TW" | "en";

export const LANGUAGE_OPTIONS: { value: Lang; label: string }[] = [
  { value: "zh-TW", label: "繁體中文" },
  { value: "en", label: "English" },
];

const zh = {
  // 檔位
  tierBest: "高",
  tierBalanced: "中",
  tierFast: "低",
  tierDefault: "跟隨預設",

  // Onboarding（BYOK 引導）
  onboardTitle: "還差最後一步：貼上 API key 就能開玩",
  onboardIntro:
    "本 App 不代管模型——你自備一把 OpenRouter key，一把通吃多家模型（角色與 GM 可以用不同檔位）。",
  onboardStep1: "註冊 OpenRouter",
  onboardStep1Btn: "開啟註冊頁",
  onboardStep2: "儲值小額（最低 5 美元，用多少扣多少，不會自動扣款）",
  onboardStep3: "建立一把 API key 並貼到下方",
  onboardStep3Btn: "開啟 API key 頁",
  onboardCost: "費用有多高？以平衡檔粗估，5 美元約可玩 3 小時；改用「快速省額度」檔更便宜。",
  onboardSaveBtn: "儲存並開玩",
  onboardCliHint:
    "已自行安裝並登入官方 CLI 的進階使用者，也可以改到左下角「設定 → AI 連線」啟用 CLI 訂閱模式。",

  // 首開語言選擇
  firstRunTitle: "歡迎來到 Table Tavern",
  firstRunIntro: "選擇介面與範例桌的語言，之後隨時可在「設定 → 外觀」更改。",
  firstRunStart: "開始",

  // 設定視窗
  settingsBtn: "設定",
  appearanceTab: "外觀",
  aiTab: "AI 連線",
  closeBtn: "關閉",
  textSizeLabel: "文字大小",
  textSizeXS: "更小",
  textSizeS: "小",
  textSizeM: "中",
  textSizeL: "大",
  textSizeXL: "更大",
  languageLabel: "語言 Language",
  transportLegend: "連線方式",
  transportApi: "API 直連（OpenRouter，標準）",
  cliSubscriptionSuffix: "（訂閱模式，進階）",
  cliDetected: "已偵測：{version}",
  cliNotDetected: "未偵測到；請自行安裝並登入官方 CLI，App 不代辦",
  riskTitle: "啟用前請了解具體風險：",
  risk1:
    "供應商條款禁止第三方工具使用訂閱憑證；Google 已對同類工具的使用者執行帳號停權且申訴無果；Anthropic 保留不經通知執法的權利。",
  risk2: "多角色扮演的用量形狀與條款所述「一般個人使用」有可見差距，可能觸發限流或審查。",
  risk3: "在訂閱模式下生成違反該供應商內容政策的內容，風險疊加。",
  risk4: "後果由你自己的帳號承擔。",
  riskAccept: "我已了解上述風險，仍要以自己的帳號啟用",
  riskRequired: "啟用 CLI 訂閱模式前，請先勾選風險告知確認",
  apiKeyLabel: "OpenRouter API key",
  tierModelApiLabel: "「{tier}」檔位模型（可挑可貼，任何 OpenRouter 模型 id）",
  tierModelCliLabel: "「{tier}」檔位模型",
  cliDefaultOption: "預設（CLI）",
  customModelOption: "自訂模型 id…",
  customModelPlaceholder: "完整模型 id",
  cliCatalogClaude: "清單讀自 Claude CLI 的本機模型快取；沒列到的用「自訂」手填",
  cliCatalogCodex:
    "清單讀自 Codex 的本機模型快取；檔位另固定對應推理力度 高→high、中→medium、低→low",
  gmTierLabel: "GM 檔位（導演與旁白用，建議選「高」）",
  maxRoundLabel: "GM 推進每回合最大發言數",
  baseUrlLabel: "自訂 base URL（進階，留空用 OpenRouter）",
  saveSettings: "儲存設定",
  saved: "已儲存",

  // 世界設定
  worldSummary: "世界設定 world.md（只進 GM 上下文，角色只知道 GM 說出口的內容）",
  worldAria: "世界設定",
  saveWorld: "儲存世界設定",
  worldbookTitle: "世界書",
  worldbookAddEntry: "新增條目",
  worldbookImport: "匯入世界書",
  worldbookExport: "匯出世界書",
  worldbookEmpty: "尚無世界書條目",
  worldbookNoKeys: "無關鍵字",
  worldbookConstant: "恆定",
  worldbookConstantLabel: "恆定（不用等關鍵字，每次發言都送進上下文——每則都佔 token、上下文會胖，只留給世界觀基本盤）",
  worldbookDisabled: "停用中",
  worldbookMoveUp: "上移",
  worldbookMoveDown: "下移",
  worldbookEdit: "編輯",
  worldbookDelete: "刪除",
  worldbookEntryTitle: "標題",
  worldbookKeys: "關鍵字",
  worldbookKeysHint: "以逗號或頓號分隔",
  worldbookContent: "內文",
  worldbookEnabled: "啟用",
  worldbookVisibility: "可見範圍",
  worldbookVisibilityGm: "GM 專有",
  worldbookVisibilityPublic: "全體公開",
  worldbookVisibilityCharacters: "指定角色",
  worldbookCharacterCount: "{n} 位角色",
  worldbookChooseCharacters: "選擇角色",
  worldbookNoCharacters: "目前沒有可選角色",
  worldbookSaveEntry: "儲存條目",
  worldbookCancel: "取消",
  worldbookEntrySaved: "條目已儲存",
  worldbookDeleteTitle: "永久刪除條目",
  worldbookDeleteConfirm: "確定要永久刪除「{title}」嗎？此操作不可復原。",
  worldbookImported: "已匯入 {n} 條",
  worldbookReadError: "無法讀取 JSON 檔案",
  worldbookJson: "JSON",

  // 角色卡
  editCardSummary: "編輯「{name}」角色卡",
  publicLabel: "公開設定（所有人認識的它）",
  privateLabel: "私有設定（只進本角色與 GM 的上下文）",
  tierLabel: "檔位",
  saveCard: "儲存角色卡",
  showImageLabel: "顯示角色圖片（關閉改回 emoji 頭像）",
  importCard: "匯入卡",
  importCardHint: "匯入 SillyTavern 角色卡（PNG 或 JSON），原檔會保留在該桌目錄",
  archiveSectionTitle: "隱藏角色",
  archiveCharacter: "收起角色",
  restoreCharacter: "還原",
  deleteCharacter: "刪除",
  deleteCharacterTitle: "永久刪除角色",
  deleteCharacterConfirm: "確定要永久刪除「{name}」嗎？角色卡與圖片將一併刪除，且不可復原。",

  // 主畫面
  newTable: "＋ 開新的一桌",
  newTableName: "新的一桌",
  tableListAria: "桌列表",
  renameHint: "點一下改名",
  tableNameAria: "桌名",
  exportTranscript: "匯出紀錄",
  exportTranscriptHint: "將全部場景匯出成 Markdown，儲存位置自選",
  exportFileName: "{table} 跑團紀錄 {stamp}",
  sceneAdvance: "換幕",
  sceneAdvanceHint: "結束本幕：把目前紀錄壓成前情提要，開新幕",
  sceneTooLongHint: "紀錄較長，建議換幕壓縮前情",
  pastScenes: "前幕（{count}）",
  sceneLabel: "第 {n} 幕",
  sceneWithTitle: "第 {n} 幕：{title}",
  exportScene: "匯出本幕",
  sceneExportFileName: "{table} 第 {n} 幕 {stamp}",
  hideActs: "隱藏",
  backToNow: "返回",
  sidebarResizerAria: "調整側欄寬度",
  castAria: "角色",
  castHint: "點名「{name}」接話",
  newCharacterAria: "角色名稱",
  newCharacterPlaceholder: "新角色名稱",
  createCard: "建卡",
  reservedNameError: "「GM」與「玩家」是保留名稱，請換一個角色名",
  messagesAria: "對話",
  typing: "{name} 正在打字",
  gmCallOn: "GM 請「{name}」發言",
  composerAria: "玩家輸入",
  composerPlaceholder: "對「{name}」發言…",
  composerNoCharacter: "先建立一個角色",
  send: "送出",
  characterFallback: "角色",
  requestReplyBtn: "請{name}發言",
  requestReplyHint: "不輸入玩家發言，直接請被點名的角色接話",
  gmNarrate: "GM 旁白",
  gmNarrateHint: "請 GM 插入一段場景旁白（GM 讀得到世界設定與全部角色卡）",
  gmAdvance: "GM 推進",
  gmAdvanceHint: "GM 點名下一位角色接話並接力推進，遇「輪到玩家」或達每回合上限即停",
} as const;

export type MsgKey = keyof typeof zh;

const en: Record<MsgKey, string> = {
  tierBest: "High",
  tierBalanced: "Medium",
  tierFast: "Low",
  tierDefault: "Follow default",

  onboardTitle: "One last step: paste an API key and start playing",
  onboardIntro:
    "This app never proxies models — you bring your own OpenRouter key, and one key covers many providers (characters and the GM can use different tiers).",
  onboardStep1: "Sign up for OpenRouter",
  onboardStep1Btn: "Open sign-up page",
  onboardStep2: "Top up a small amount (US$5 minimum, pay-as-you-go, no auto-charge)",
  onboardStep3: "Create an API key and paste it below",
  onboardStep3Btn: "Open API keys page",
  onboardCost:
    "How much does it cost? Roughly, US$5 buys about 3 hours of play on the Medium tier; the Low tier is even cheaper.",
  onboardSaveBtn: "Save and play",
  onboardCliHint:
    "Advanced users who already installed and logged into an official CLI can instead enable CLI subscription mode under “Settings → AI Connection” in the lower left.",

  firstRunTitle: "Welcome to Table Tavern",
  firstRunIntro:
    "Choose the language for the interface and the sample table — you can change it anytime under Settings → Appearance.",
  firstRunStart: "Start",

  settingsBtn: "Settings",
  appearanceTab: "Appearance",
  aiTab: "AI Connection",
  closeBtn: "Close",
  textSizeLabel: "Text size",
  textSizeXS: "Extra small",
  textSizeS: "Small",
  textSizeM: "Medium",
  textSizeL: "Large",
  textSizeXL: "Extra large",
  languageLabel: "Language 語言",
  transportLegend: "Connection",
  transportApi: "Direct API (OpenRouter, standard)",
  cliSubscriptionSuffix: " (subscription mode, advanced)",
  cliDetected: "Detected: {version}",
  cliNotDetected:
    "Not detected; install and log into the official CLI yourself — the app won't do it for you",
  riskTitle: "Understand the concrete risks before enabling:",
  risk1:
    "Provider terms forbid third-party tools from using subscription credentials; Google has suspended accounts of users of similar tools with appeals denied, and Anthropic reserves the right to enforce without notice.",
  risk2:
    "Multi-character roleplay produces a usage pattern visibly different from the “ordinary personal use” described in the terms, which may trigger rate limits or review.",
  risk3:
    "Generating content that violates the provider's content policy while on subscription mode compounds the risk.",
  risk4: "The consequences fall on your own account.",
  riskAccept: "I understand the risks above and still want to enable this with my own account",
  riskRequired: "Please confirm the risk notice before enabling CLI subscription mode",
  apiKeyLabel: "OpenRouter API key",
  tierModelApiLabel: "“{tier}” tier model (pick or paste any OpenRouter model id)",
  tierModelCliLabel: "“{tier}” tier model",
  cliDefaultOption: "CLI default",
  customModelOption: "Custom model id…",
  customModelPlaceholder: "Full model id",
  cliCatalogClaude:
    "List comes from the Claude CLI's local model cache; use “Custom” for anything missing",
  cliCatalogCodex:
    "List comes from the Codex local model cache; tiers also map to reasoning effort High→high, Medium→medium, Low→low",
  gmTierLabel: "GM tier (for directing and narration; “High” recommended)",
  maxRoundLabel: "Max speakers per GM-advance round",
  baseUrlLabel: "Custom base URL (advanced; leave empty for OpenRouter)",
  saveSettings: "Save settings",
  saved: "Saved",

  worldSummary:
    "World settings world.md (GM-only context; characters only know what the GM says out loud)",
  worldAria: "World settings",
  saveWorld: "Save world settings",
  worldbookTitle: "World Book",
  worldbookAddEntry: "Add entry",
  worldbookImport: "Import world book",
  worldbookExport: "Export world book",
  worldbookEmpty: "No world book entries yet",
  worldbookNoKeys: "No keywords",
  worldbookConstant: "Constant",
  worldbookConstantLabel: "Constant (always in context, no keyword needed — costs tokens on every turn, so reserve for core world facts)",
  worldbookDisabled: "Disabled",
  worldbookMoveUp: "Move up",
  worldbookMoveDown: "Move down",
  worldbookEdit: "Edit",
  worldbookDelete: "Delete",
  worldbookEntryTitle: "Title",
  worldbookKeys: "Keywords",
  worldbookKeysHint: "Separate with commas or ideographic commas",
  worldbookContent: "Content",
  worldbookEnabled: "Enabled",
  worldbookVisibility: "Visibility",
  worldbookVisibilityGm: "GM only",
  worldbookVisibilityPublic: "Public",
  worldbookVisibilityCharacters: "Specific characters",
  worldbookCharacterCount: "{n} characters",
  worldbookChooseCharacters: "Choose characters",
  worldbookNoCharacters: "No characters available",
  worldbookSaveEntry: "Save entry",
  worldbookCancel: "Cancel",
  worldbookEntrySaved: "Entry saved",
  worldbookDeleteTitle: "Permanently delete entry",
  worldbookDeleteConfirm: "Permanently delete “{title}”? This cannot be undone.",
  worldbookImported: "Imported {n} entries",
  worldbookReadError: "Could not read the JSON file",
  worldbookJson: "JSON",

  editCardSummary: "Edit “{name}” character card",
  publicLabel: "Public profile (what everyone knows about them)",
  privateLabel: "Private profile (only this character and the GM see it)",
  tierLabel: "Tier",
  saveCard: "Save character card",
  showImageLabel: "Show character image (off = emoji avatar)",
  importCard: "Import",
  importCardHint:
    "Import a SillyTavern character card (PNG or JSON); the original file is kept in the table folder",
  archiveSectionTitle: "Hidden characters",
  archiveCharacter: "Hide character",
  restoreCharacter: "Restore",
  deleteCharacter: "Delete",
  deleteCharacterTitle: "Permanently delete character",
  deleteCharacterConfirm:
    "Permanently delete “{name}”? The character card and image will both be deleted. This cannot be undone.",

  newTable: "＋ New table",
  newTableName: "New table",
  tableListAria: "Table list",
  renameHint: "Click to rename",
  tableNameAria: "Table name",
  exportTranscript: "Export transcript",
  exportTranscriptHint: "Export every scene as Markdown to a location you choose",
  exportFileName: "{table} transcript {stamp}",
  sceneAdvance: "New act",
  sceneAdvanceHint: "End this act: compress the current log into a recap and start a new act",
  sceneTooLongHint: "This log is getting long — consider starting a new act to compress it",
  pastScenes: "Past acts ({count})",
  sceneLabel: "Act {n}",
  sceneWithTitle: "Act {n}: {title}",
  exportScene: "Export this act",
  sceneExportFileName: "{table} act {n} {stamp}",
  hideActs: "Hide",
  backToNow: "Back",
  sidebarResizerAria: "Resize sidebar",
  castAria: "Characters",
  castHint: "Call on “{name}” to speak",
  newCharacterAria: "Character name",
  newCharacterPlaceholder: "New character name",
  createCard: "Create",
  reservedNameError: "“GM” and “玩家” (player) are reserved names — pick another one",
  messagesAria: "Conversation",
  typing: "{name} is typing",
  gmCallOn: "GM asks “{name}” to speak",
  composerAria: "Player input",
  composerPlaceholder: "Speak to “{name}”…",
  composerNoCharacter: "Create a character first",
  send: "Send",
  characterFallback: "the character",
  requestReplyBtn: "Ask {name} to speak",
  requestReplyHint: "Skip the player line and have the selected character speak directly",
  gmNarrate: "GM narration",
  gmNarrateHint:
    "Ask the GM to insert scene narration (the GM can read the world settings and every character card)",
  gmAdvance: "GM advance",
  gmAdvanceHint:
    "The GM calls on the next character and keeps the scene moving, stopping at the player's turn or the per-round cap",
};

const MESSAGES: Record<Lang, Record<MsgKey, string>> = { "zh-TW": zh, en };

let lang: Lang = "zh-TW";

export function normalizeLang(value: unknown): Lang {
  return value === "en" ? "en" : "zh-TW";
}

export function setLang(next: Lang) {
  lang = next;
}

export function t(key: MsgKey, params?: Record<string, string | number>): string {
  let text: string = MESSAGES[lang][key];
  if (params) {
    for (const [name, value] of Object.entries(params)) {
      text = text.split(`{${name}}`).join(String(value));
    }
  }
  return text;
}
