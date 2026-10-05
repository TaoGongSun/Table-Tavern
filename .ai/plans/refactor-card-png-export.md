# refactor-card-png-export — 規格

2026-08-15 使用者拍板＋Sol（GPT-5.6）二輪查證收斂同意；逐字稿在 Codex app 同名 Companion 串。

## 目標與格式選擇

匯出分三階，全部單一 PNG。為何選 PNG：主要情境是玩家互傳，metadata 剝除是全生態共同前提（ST 卡過重壓平台一樣壞，圈內靠 Discord 附件／catbox 等保留原檔通道），而 zip 要寫各 OS 縮圖外掛才看得到封面，PNG 圖素天生是封面。

1. **ST 相容卡**：已有（card-export 結案，tEXt `chara` chunk），不動。
2. **重構卡 PNG**：RefactorOutcome 封進自家私有 chunk；沿用「副檔名決定格式」慣例，.json 照舊可存。
3. **重構卡＋角色圖 PNG**：#2 再加角色圖 chunk。

三階靠檔名尾碼區分、封面不蓋徽章；尾碼僅供玩家辨識，格式判定只認 chunk。

## chunk 與 manifest 規格

- 兩種 chunk：manifest 一個＋圖片 chunk 可重複。四字名取 ancillary＋private＋safe-to-copy（例 `ttRd`／`ttId`，實作定案）；payload 開頭放 magic＋版本，防私有名稱碰撞。
- manifest JSON：`{ format, version, outcome, applied?, assets }`；assets 每筆 `{ asset_id, outcome_index, kind: portrait|avatar, mime, length, hash }`，**不含檔名欄位**（路徑注入面直接消滅）。
- 圖片 chunk 直存原始 bytes（免 base64）；配對只認 asset_id，不依賴 chunk 順序（PNG 編輯器可重排 ancillary chunk）。
- 匯出底圖剝掉既有 `chara`／`ccv3`＋舊版自家 chunk，避免新舊疊包。#2/#3 不寫 `chara`，ST 誤吃會乾淨報「找不到角色卡」。
- 匯入單一拖放入口，按 chunk 嗅探分流 `chara`／`ccv3`／自家。

## 地基：套用映射持久化（前置包）

現況 `worlds/<id>/refactor-outcome.json` 存原始 outcome（refactor.rs `write_refactor_outcome`），outcome_index → 建卡 character_id 的映射只活在 `RefactorApplyResult` 記憶體，角色改名後 #3 匯出配不回圖。改法：

- 落檔擴充為 `{ format, version, outcome, applied }`；applied 含 outcome_index → character_id 映射（走 person 條目沒建卡的 index 標記無卡）＋玩家卡資訊；二次套用整份覆寫。
- 讀取端同時接受舊版裸 RefactorOutcome，既有 JSON 重構卡不失效。
- 接收端套用要拿明確 index → id 映射，不靠 character_ids 陣列順序（勾選可為不連續）。
- `applied.player_card_id` 屬來源桌本地資訊，跨桌重現玩家選擇用邏輯 player_index。

## #3 範圍

- 角色圖＝各角色目前的全身圖＋avatar.png 裁切頭像；排除 gen-gallery（個人生成資產、體積大、桌子運行不需要）。
- 卡內容凍結為 outcome 的角色；套用後手動新增的角色屬世界編輯，不進卡。

## 匯入驗證（信任邊界）

整包原子驗證：CRC、hash、重複／缺漏 asset 對帳、張數與總大小上限、圖片 MIME 與尺寸；上限數值實作時定。

## 驗收

- 刻意做一張數十 MB 大卡實測：macOS Finder／QuickLook、Windows Explorer、ST 匯入拒收訊息、本應用匯出→匯入往返。
- 匯出完成訊息顯示檔案大小（Discord 免費附件上限目前約 10MB）。

## 施工計畫（2026-10-06 子代理擬；Sol 兩輪審查收斂）

### 共用格式（A 起定死，B/C 沿用）

- 封套 JSON（落檔、.json 匯出、PNG manifest 同一形狀）：`{ "format": "table-tavern-refactor-card", "version": 1, "outcome": {…}, "applied"?: { "characters": [{ "outcome_index", "character_id": string|null }], "player_index": number|null, "player_card_id"?: string|null } }`。
- applied 完整性（Rust 與前端同一套規則，信任邊界在 Rust）：characters 恰好覆蓋 0..角色數 每個 index 各一次；character_id 為 null 或非空字串，非 null id 互不重複；player_index 為 null 或指向 character_id 非 null 的 index。違反任一條整份拒收。
- 對外（.json 匯出、PNG manifest）去掉 `player_card_id`（來源桌本地資訊）；`character_id` 保留、接收端不用。
- PNG：manifest chunk `ttRd`＝`TTRC`＋版本 byte＋封套 JSON（多 `assets` 欄）；圖片 chunk `ttAs`＝`TTAS`＋版本 byte＋asset_id 長度 byte＋asset_id＋原始 bytes；皆插在 IEND 前。asset：`{ asset_id, outcome_index, kind: "portrait"|"avatar", mime: "image/png", length, hash: sha256 hex }`。
- 上限（匯出、匯入同一組常數）：檔案總長 ≤ 300 MiB；manifest ≤ 32 MiB；張數 ≤ 2×角色數；單張 ≤ 32 MiB、總和 ≤ 256 MiB；單張寬高各 1–8192 且像素數 ≤ 24M；解碼記憶體預算 ≤ 192 MiB（png crate `Limits`），一次只解一張、緩衝區重用。
- 圖片完整驗證（共用函式，匯入素材、匯出素材、匯出底圖、`import/images.rs` 的存圖都可用）：PNG magic；chunk 結構邊界與全部 CRC；首 chunk IHDR 且合法；至少一個 IDAT 且連續；恰一個零長 IEND、其後無任何資料；再用 png crate（已在 Cargo.lock，改為直接依賴）帶 `Limits` 完整解碼一次，截斷／壞壓縮流／尺寸超限皆拒。
- 匯出底圖：GM 圖 → 第一位已建卡角色全身圖 → 1×1 透明圖〔模型判斷·未裁決〕；候選圖過不了完整驗證或超過底圖上限（同單張上限）就換下一個；一律剝 `chara`／`ccv3`（tEXt/zTXt/iTXt）與 `ttRd`／`ttAs`。

### 包 A：套用映射持久化（首包）

- `refactor/types.rs`：加 `RefactorAppliedMap`／`RefactorCardFile`；`RefactorApplyResult` 加 `character_map: Vec<Option<String>>`（index→id）。
- `refactor/apply.rs`：建卡迴圈填映射；尾端落檔改寫封套（含 player_index＋player_card_id），二次套用整份覆寫。
- 新 `refactor/card_file.rs`：讀封套或舊裸 RefactorOutcome（裸檔 → applied=None）；applied 完整性驗證；對外序列化去本地欄位。
- `refactor_export_saved` 的 .json 改輸出對外封套（舊裸存檔輸出成無 applied 的封套）。
- 前端 `parseRefactorOutcome`：認封套（format 不符拒收；version > 1 給「版本較新」訊息）與舊裸 JSON，回 `{ outcome, applied }`，applied 同規則驗。`defaultRefactorSelection` 有 applied 時預設勾來源桌建卡的那幾位＋其 player_index；接收桌已有玩家卡就沿用現有整批拒套（PlayerCardExists），不靜默取消玩家位。
- 測試：cargo（封套 round-trip、舊裸檔、不連續勾選映射、二次套用覆寫、對外去 player_card_id、applied 六種違規各一拒收）；vitest 同規則＋預設勾選重現。既有讀存檔當裸 outcome 的測試改讀封套。

### 包 B：共用 codec＋PNG 匯出（#2/#3）

- 封裝驗證器（`refactor/card_png.rs`，匯入與匯出自檢共用，逐項皆整包拒收）：
  - PNG 層：上列圖片結構規則套在整個檔（CRC、IHDR、IDAT、唯一零長 IEND 無尾資料）；檔案總長上限。
  - `ttRd` 恰一個；payload 開頭 `TTRC`＋版本 byte＝1；manifest 長度上限、UTF-8 JSON、format 相符、封套 version 只接受 1。
  - 每個 `ttAs` payload 開頭 `TTAS`＋版本 byte＝1，asset_id 長度 byte 不越界。
  - asset_id 限 `[a-z0-9_-]{1,32}`、manifest 內不重複；`(outcome_index, kind)` 不重複且 outcome_index < 角色數；kind 只收 portrait／avatar；mime 只收 image/png。
  - manifest 與圖片 chunk 一對一：每筆 asset 恰一個同 id 的 chunk，無缺、無重複 chunk、無孤兒 chunk；chunk 實際 bytes 長度＝length、SHA-256＝hash。
  - 張數、單張、總和上限；每張圖過圖片完整驗證；applied 完整性。
- 新 `refactor/card_png.rs`：encode、decode＋上述驗證、剝 chunk、上限常數；圖片完整驗證放在 import 側共用（`import/card_io.rs` 的 `png_chunk`／`crc32`／`PNG_MAGIC` 升 `pub(crate)`）。B 負責 codec 全部拒收測試：逐條驗證項各一拒收案（含沒 IDAT、IDAT 截斷、壞 zlib、IEND 非零長／重複／後有尾資料、CRC 錯、尺寸與像素超限、解碼預算超限）。
- `refactor_export_outcome(outcome, path)`：.json 照舊、其餘 #2。`refactor_export_saved(world_id, path, with_images)`：.json／#2／#3；`with_images` 只接受 .png，路徑是 .json 直接回錯，不靜默丟圖。#3 素材＝applied 映射角色目前的 `<id>.png`＋`<id>.avatar.png`（排除 gen-gallery；角色已刪略過）；任一素材過不了驗證或超上限，整個匯出拒絕並點名角色。寫檔前把產出的 bytes 走一次匯入驗證器，保證不產出自己拒收的卡。回傳寫出位元組數。
- 舊裸存檔沒有映射 → #3 回固定訊息「這張重構卡是舊版存檔，沒有角色對應，附不了角色圖；可改匯出不含角色圖的版本」（不建議在原桌重套）〔模型判斷·未裁決〕。
- 前端：存檔對話框（不含圖：PNG＋JSON、預設 .png；含圖：只 PNG）；世界書 ⋯ 選單「匯出重構卡」下加「匯出重構卡（含角色圖）」；預設檔名尾碼 `-重構卡.png`／`-重構卡+角色圖.png`（在地化）；完成後 revealItemInDir＋狀態列顯示檔案大小。i18n 十語系。

### 包 C：匯入、暫存、套用競態、單一入口

- `refactor_card_open`：前端以 raw IPC body 送原始 bytes；後端嗅探：有 `ttRd` → 自家；只有 `chara`／`ccv3` → 固定錯誤「這是角色卡，請用陣容欄匯入」；非 PNG → JSON 封套／裸 outcome。全部驗證通過才暫存，任一失敗整包拒收、零寫入。回傳 `{ card_json, assets: [{ outcome_index, kind }], token }`；outcome 交前端 `parseRefactorOutcome` 再驗一次。manifest 沒檔名欄位，落檔路徑一律由新建角色 id 決定。
- 暫存單槽：內容＝world_id＋token＋角色名清單＋已驗圖片。
  - 開新卡覆蓋舊槽（舊 token 失效）。`refactor_card_release(world_id, token)` 只在 world＋token 都相符時清，舊結果卡關閉清不到新卡。
  - `refactor_apply` 選填 `asset_token`：command 一進來（取整桌鎖之前）就以 world＋token 把素材從槽中取出、由這次呼叫持有到結束；取不到（已被覆蓋／釋放）就整批拒套。之後關卡、開新卡、換桌都碰不到這份素材。前端：apply 排隊等回合期間不釋放（closeRefactor 在 busy 時不送 release）；結果卡關閉或卸載且無在途套用才 release。
  - 傳入 outcome 角色數與名字須與暫存逐一相符，否則整批拒套。
  - 取走後套用失敗（PlayerCardExists、介面 preflight 拒套、名字不符等，未寫入任何東西的 Err）：素材放回槽，但只在槽是空的時候放回；槽裡已有較新的卡就丟棄這份，回固定錯誤，前端清掉結果卡狀態並提示重新開檔。受控測試：取走後拒套→重試成功；取走後拒套期間已開新卡→重試得到重新開檔錯誤、新卡槽不受影響。
  - 受控測試：開 A→開 B→關 A（B 仍在、A token 失效）；apply 已取出素材後關卡／開新卡／換桌，套用照樣用原素材完成且新卡槽不受影響。
- 寫圖與失敗收尾：寫圖在 `apply()` 內、同一整桌鎖下、所有其他套用動作之後進行，依 `character_map` 寫進新建卡（走 PNG 檢查的 save_character_image/avatar）。寫圖失敗不回 Err：其餘套用已成立，照常記收據（撤銷刪角色時連圖一併刪），summary 帶 `images_failed` 與失敗角色名，前端在完成訊息列出。寫圖逐張進行，一張失敗不停下其餘圖片。用 cfg(test) 注入「第二張圖寫入失敗」驗證：角色與條目落地、收據存在、撤銷乾淨、第一張與第三張以後的圖都在。
- 單一入口：`useImportController.importFile` 在 `Array.from` 前，先以前端純函式走 PNG chunk 表嗅 `ttRd`（vitest 覆蓋）；命中就不轉數字陣列，打開世界設定並交出 pending file，pending 綁來源 world＋請求世代，WorldEditor 只在 world 與世代相符時接手，換桌即作廢。`decode_png_character` 沒 `chara` 時讀 `ccv3`。結果卡摘要多一行「附 N 張角色圖」。i18n 十語系。
- 測試：cargo 往返（匯出 #3 → 開啟 → 套用到另一桌 → 圖落在正確角色、改名不影響）＋上述競態與失敗注入；vitest 嗅探、pending 世代。

### 包 D：實測

- 測試通道（整機一把鎖，啟動前 ps 確認無他人實例，用完 quit）：西幻卡匯入 → 零額度餵產物 JSON 套用 → 兩位角色上圖 → 匯出三階 → 新桌匯原卡 → 匯入 #3 PNG → 套用 → 截圖確認圖到位；陣容欄丟 #3 PNG 走單一入口。
- 數十 MB 大卡：本機 `qlmanage -t`／`sips` 看 macOS 縮圖。Windows Explorer 與 ST 匯入拒收訊息排實測佇列；這兩項未驗，本案不標實測完成。
