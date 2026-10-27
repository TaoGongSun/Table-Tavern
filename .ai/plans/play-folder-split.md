# play-folder-split 定案

三方（主線、Sol、Grok）共識，2026-10-28。

## 分法（依子功能，純搬檔、git mv、直改 import 不留轉出殼）
- `play/chat/`（聊天回合流程）：useChatController、useSmartFreeNotices（新）、undo-stack(+test)、TurnFailedDialog(+test)、chat-marker-events／chat-reload-race／chat-send-failure／chat-smart-free-notices 四支測試。
- `play/scene/`（換幕與容量）：useSceneActions、scene-actions-lock.test、scene-budget(+test)、useSceneBudget、scene-budget-refresh.test。
- `play/` 根層：PlayView(+test)、stream-display(+test)、ActReader、useGmOnlyTable、gm-only-table.test、world-info-notices(+test)、world-info-reset、useTurnWait(+test)。
- 依賴單向：chat、scene 只往上引用根層 world-info，兩者互不引用；根層 PlayView 引 chat（ChatNotice 型別）與 scene（scene-budget）。
- world-info 三檔留根層：chat、scene、App 共用的 play 公用部分。
- useTurnWait 留根層：play 內無人引用，呼叫端全在桌外（App、import、refactor），不讓桌外功能依賴聊天子目錄。

## useChatController
- 回合狀態機共用一整包 ref（世代、忙碌鎖、寫入追蹤、動作 id 等），不沿行號硬拆。
- 只抽換模提示訂閱到 `chat/useSmartFreeNotices.ts`（notices state、監聽 effect、可見過濾、ChatNotice 型別）：generation／ownTurns 傳原 ref，呼叫點在 render 期世代遞增之後，effect 與 deps 原樣搬（visibleNotices deps 維持 [notices, worldId, scene]）。以既有 chat-smart-free-notices.test 經 controller 入口驗收。
