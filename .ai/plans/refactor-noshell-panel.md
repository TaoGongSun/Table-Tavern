# refactor-noshell-panel — 計畫

交接：[handoffs/archive/refactor-noshell-panel.md](../handoffs/archive/refactor-noshell-panel.md)

## 查到的事實

- 盤點的 playable 判定只存在重構流程裡，不會寫進桌子。桌上能用的訊號只有兩個：`state.refactor_mode`，以及 `interface-shell.html`。`refactor_mode` 在 `apply` 時寫入，只要產物帶 mode 就寫，跟有沒有勾介面無關；殼檔只在 `interface.shell` 非空時寫。
- bcd、nhs 產物：`mode=interface`、`interface.shell=null`、rules／guide 空；tf、wf 有 shell。
- 原 `pickCardShell`：interface 桌沒骨架、或骨架過不了卡腳本時，會退到「近 10 則掃 raw＋空桌掃卡片開場白」。NorthHall 的「加载中...」來自開場白那一支。
- 原 `apply` 只在 characters 模式刪殼檔，而且吞掉錯誤。收據只記 `interface_shell_created`；被刪或被覆寫的舊殼、`refactor_mode` 都退不回來。
- 入口（Sol 已核對）：`AppWorkspace.tsx:229,369` 的按鈕與覆蓋層都看 `shellReady`；`openIfDrawable` 繞不過 `shellReady`；apply／undo 之後都會呼叫 `refreshShell`；匯出 saved outcome 讀原產物，不受清理與 undo 影響（維持）。
- 原 `extractShell`：`FENCE_REGEX` 只吃英文字母標記，只收 `html`，或空白標記且內容是頁面；bare fallback 會從被拒收的圍欄內切出頁面。NorthHall 型卡用 ```text 包 `<!DOCTYPE html`。

## 定案範圍

- 這次大改之前建立的桌不保證能用、不做相容，只確保新桌好用〔作者裁決 2026-10-02〕。mode=null 的桌程式行為照現狀，不另寫測試保證。
- interface 桌沒有有效重構殼就不給面板，direct-first、近 10 則掃 raw、空桌開場白三條退路一起關〔作者裁決 2026-10-02〕。
- 套用後沒產殼時清掉 `interface-shell.html`；只處理殼檔，`state.json` 的 incremental／rules／guide 不動，因為沒有來源歸屬資訊，不能整批清〔模型判斷·未裁決〕。
- 沒勾介面跟沒產殼一視同仁，判斷看「這次套用後有沒有新殼」〔模型判斷·未裁決〕。
- 收據記 `refactor_mode` 的原值，undo 一起退回〔模型判斷·未裁決，主線決定納入〕。

## 做法（已實作）

### 1. 選路（`card-shell-route.ts`）
- 骨架 null、空字串、純空白都當沒有；`tableMode === "interface"` 且沒有骨架 → 回 null，這個短路排在 direct-first 之前。
- `LEGACY_PAGE_SHELL` 判斷與它的四個測試刪除：不再支援辨識舊產殼格式。
- controller 的 `tableMode`／`refactorShell` 註解改成現況。

### 2. 面板開著時殼消失（`useCardInterfaceController.ts`）
`shellReady` 由 true 轉 false 時 `setCardUiOpen(false)`：面板立即關閉，之後殼回來（例如 undo）也不會自己跳開。

### 3. Rust `apply` 清殼（`refactor/apply.rs`）
- `new_shell`：mode 不是 characters、有勾介面、產物 interface 存在，而且 shell trim 後非空。寫殼分支也用同一個值，純空白殼不會先刪舊殼又寫進空白殼。
- 帶 mode 的產物而且 `new_shell` 為 None → 用 `commit_world_remove` 刪殼。時機在全部 preflight（mode 解析、介面路徑正規化、玩家卡衝突、uid／order 溢位）之後、第一筆寫入之前。檔案不存在算成功，其餘錯誤往上傳，刪不掉時整批失敗、桌上沒有其他改動。沒有 mode 的產物不碰殼檔。
- characters 分支原本吞錯誤的刪殼併進同一步。

### 4. 收據與 undo（`receipts.rs`、`commands/refactor.rs`）
- 新增 `snapshot_refactor`：狀態與殼內容都必須讀得到，讀失敗回 Err、不套用（不把讀失敗當成沒有殼）。`refactor_apply` 指令改用它。
- 收據新增 `interface_shell_restore: Option<String>`：套用前有殼，而且套用後殼被刪或內容不同時，記下原內容（也涵蓋覆寫）。
- 收據新增 `refactor_mode: Option<ModeUndo { before: Option<String> }>`：外層有值＝這次改了標記，`before` 為 None＝原本沒有標記。不用 `Option<Option<String>>`，因為 serde 往返分不出差別。
- 只有清殼或只有 mode 變更也產收據。
- undo 第 0 步先處理殼與 mode：`interface_shell_created` 刪新殼，`interface_shell_restore` 寫回原殼，`refactor_mode` 寫回原值，全部用 `?`。失敗時回 Err，磁碟上的收據不彈出，其餘域（角色、世界書、機制、帳本）還沒動，可以重按。不承諾整批交易：殼已寫回而 mode 寫失敗時，殼停在寫回後的狀態，重按會再做一次（兩步都可重做）。

### 5. undo 後刷新狀態樹（`App.tsx`）
復原後一律 `tableState.refresh()`，不再只在 `removed_opening` 時重讀。

### 6. extractShell（`interface-card.ts`）
- `FENCE_REGEX` 改成 ```` ```([^\r\n`]*)\r?\n([\s\S]*?)``` ````，標記取 trim 後的第一個詞、轉小寫。
- 標記為 `html` 直接收；其他任何標記，內容以頁面起點開頭才收。頁面起點為 `<!DOCTYPE\s+html\b`、`<html(?=[\s>])`，圍欄另加 `<body(?=[\s>])`；`<htmlfoo>`、`<bodyguard>` 不算。
- bare fallback 的定位與回傳都用「遮掉所有已辨識圍欄後的文字」，起點規則與圍欄共用。

## 依作者裁決不做（〔作者裁決 2026-10-02〕：大改前的桌不必管）

- Sol 第 1 輪必改 1 裡 `LEGACY_PAGE_SHELL` 的正規化：改成刪除整段判斷。
- Sol 第 1 輪必改 3 的舊收據相容測試（新欄位的 `serde(default)` 是「None 不落檔」的格式需要，照留）。
- 測試矩陣的「舊整頁殼」那一軸，以及 mode=null 的行為保證測試。

## 接受的限制：殘留機制值（不清）

interface 模式重跑沒產殼時，上一次的 `incremental=true`、卡專屬 rules、guide 會留在 state.json：
- 提示詞照舊帶狀態更新協定、guide，以及「介面由 App 畫」聲明。狀態值照常記帳，顯示在頂部狀態欄。
- `mechanism/apply.rs::apply_insert` 能建立不存在的路徑：模型照舊 guide 回報已淘汰的欄位時，該欄位會重新長回狀態樹；舊 rules 也會套到新樹裡同名的欄位。`stale_rules_survive_rerun_and_insert_regrows_retired_field` 把現行行為釘住。
- 第一次重構就沒產殼的桌（bcd、nhs）三者原本就是預設值，沒有殘留。

## 測試（已補）

- **選路**（`card-shell-route.test.ts`）：interface × 骨架 {null、空字串、純空白} × {空桌、最新 direct-first、近十則 fallback} 一律 null；有效骨架的空桌開場白、direct-first、填骨架、骨架過不了退回掃 raw、骨架內含 HTML 片段；characters／undefined 一律 null。
- **面板**（`useCardInterfaceController.test.ts`）：面板開著時殼沒了 → uiOpen 變 false；殼回來 → shellReady 回 true、面板不自己跳開。
- **整合**（`refactor-undo-flow.test.tsx`，掛整個 App）：interface 桌沒殼時沒有介面鈕；按「復原上次匯入」後介面鈕回來，頂部狀態欄換成復原後的樹。拿掉第 5 點的修改時這個測試會失敗。
- **Rust**（`refactor/tests/shell_cleanup.rs`）：
  - 重跑沒產殼 → 殼刪除，undo 還原；未勾介面 → 殼照清；純空白殼 → 不寫空白、舊殼清掉；新殼覆寫後 undo 還原舊殼。
  - 只有清殼的套用也有收據；只有 mode 變更（null→interface、null→characters、interface→characters、characters→interface）有收據，從磁碟讀回收據後 undo 退回原值。
  - 第一次接管後 undo → 殼、mode、狀態樹都回到未重構。
  - preflight 拒絕（介面路徑衝突、玩家卡已存在）→ 殼、mode、狀態樹、角色都不變。
  - 注入刪除失敗 → apply 回 Err，狀態、世界書、角色都沒動。
  - 無關的 rules／guide／incremental 保留不變；舊 rules 殘留與 insert 長回欄位（接受的限制）。
  - 殼檔讀不出來（非 UTF-8）→ `snapshot_refactor` 回 Err。
  - undo 寫回殼失敗、或刪新建殼失敗 → Err、收據還在、mode 與狀態樹不動；排除故障後重按成功。
  - saved outcome 與套用的產物相同，undo 不動它；在新桌重匯同一份產物，結果相同（沒殼）。
- **圍欄**（`interface-card.test.ts`）：text、html5、x-html、標記後帶空白、大寫標記、CRLF、空白標記；html 標記收片段；小寫 doctype 與 `<html lang>`；多個圍欄；```json 含 `<!DOCTYPE html>` 不收也不走 bare；被拒收的圍欄以外仍有裸頁面就從圍欄外切；`<htmlfoo>`／`<bodyguard>` 反例。
