# 角色卡路徑匯入時，機制帳本把認不出的條目報成已接管

Status: 已結案（2026-10-27，Sol／Grok 驗收通過，已進 main）

## 根因
兩個寫入者對 EJS 條目各判一次：`data/worldbook/book_import.rs` 把所有含 `<%` 的條目記成接管，`import/mechanism.rs` `extract_triggers` 把認不得的記成跳過；`read_ledger` 同標題取最後一筆。世界書路先寫書再寫機制，湊巧正確；角色卡路（含網頁存檔的角色卡路線）順序相反，6 條認不出的變成接管。另外只含認不出 EJS 的卡，`import_mechanism_strict` 早退不記帳，兩路都只剩 book_import 的錯誤接管。

## 定案
- 含 `<%` 的條目只由 `extract_triggers` 判決：認得記接管（沿用 `LedgerScaffoldAbsorbed`）、認不得記跳過，每條都記；`book_import` 不再替含 `<%` 的條目記帳（混合條目也一樣），匯入即停用照舊。這解決的是雙重分類，`read_ledger` 仍以標題合併，標題碰撞不在本案範圍。
- `import_mechanism_strict` 沒有狀態要寫時仍照記帳本，只有完全沒判決才直接返回。
- `refactor/apply.rs` 套用後寫的接管紀錄是合理的後續更新，不動。
- 舊桌不遷移（發佈前舊桌不用管〔作者裁決 2026-10-02〕）：角色卡路匯入過的舊桌仍顯示錯帳，同一張卡任一路重匯一次就改正（`ledger_route_tests` 有測）。
- 快照用手寫 `expected.txt`，不加 `insta`：主線裁零新依賴優先（比對量小、預期值本來就是整理過的文字，擋回歸能力相同）；Sol 原主張 insta（業界標準），Grok 主張零依賴。更新要人工跑 `UPDATE_EXPECTED=1`，值精確是 `1` 才寫檔。
- 結構只做 `card.rs` 內嵌測試抽成 `card_tests.rs`（`#[path]` 掛回，同 receipts／transport 先例）：主線裁依 STRUCTURE.md 不在功能案裡大規模搬家；Sol 原主張連 `import/tests/` 子資料夾一併做，Grok 主張只抽 card.rs。
- 乾淨測試卡進 `src-tauri/src/import/fixtures/`（兩張 JSON＋README＋expected），產生器不入版控。

## 測試
- `import/ledger_route_tests.rs`：混合卡兩路分類與條數、只有認不出 EJS 的卡、重複匯入、舊錯帳重匯改正、網頁存檔兩路、匯入後 AI 重構脈絡把認不得的列成重構目標。拿掉修法時其中 4 個會紅。
- `import/clean_cards_tests.rs`：兩張卡兩路逐字相同＋比對 expected；勇者帳本 12／6、獸人 0／0、帳本全部 `sent=false`。卡片介面分路記：桌面版世界書路只在卡帶顯示腳本時留原卡（`save_world_card`），獸人卡這路本來就沒有介面；網頁存檔的世界書路線走 strict 路徑（`save_world_card_strict`）無條件保存原卡，這個差異只限桌面匯入情境。

## 後續建議（BACKLOG 由主線立案）
- `src-tauri/src/import/` 現有 24 個 .rs 檔（11 個是測試檔）＋ fixtures/：建議另立案把測試檔收進 `import/tests/`（照 `refactor/tests/` 先例），純搬檔。

