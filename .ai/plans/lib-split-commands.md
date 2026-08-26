# lib.rs 拆進 commands/

## 目標

`src-tauri/src/lib.rs` 程式碼 3304 行、98 個 `#[tauri::command]` 攤在同一檔。按領域搬進 `commands/` 子模組，lib.rs 剩 157 行（mod 宣告＋`data_root`／`config_root`＋`run()`）。

**純搬家：任何 body 一個字不改。** 收斂重複、拆函式、改邏輯都不在本案範圍。lib.rs 那 536 行測試跟著各自的函式走，不重寫。

## 定案（2026-08-26，與 Sol 兩輪討論收斂）

| 檔案 | 行數 | 內容 |
|---|---|---|
| `commands/refactor.rs` | 608 | 16 個 `refactor_*`（最大 `refactor_survey` 98）＋`span_lookup` |
| `commands/chat.rs` | 673 | `gm_narrate` 190、`chat_with_character` 113、`gm_lane_reply` 56、`record_*_arrivals` 各 42、`load_hidden_cards`、`gm_materials`、`keepalive_lanes`、`GmNarration`／`StateUpdate`／`GmMaterials` |
| `commands/image.rs` | 636 | `generate_character_image` 109、`decode_base64` 50、`extract_image_refs` 43、gallery／頭像／base64／`path_span` |
| `src/ai_transport.rs` | 412 | `stream_turn_via_transport` 177、`prepare_lane_call` 46、`stream_via_transport` 36、`claude_cli_envs` 30、`grok_profile` 21、`ai_call_failure` 20、`chat_transport`、`claude_home_dir`、`lane_provider`、`cli_envs`、`cli_workspace` |
| `commands/scene.rs` | 359 | 逐字稿、換幕、`regenerate_scene_summary` 50、`translate_opening` 43 |
| `commands/cli_setup.rs` | 358 | `cli_install_script` 113、`install_cli` 88、`InstallMessages`、`shell_quote`、`cli_sentinel_name`、`cli_verified` |
| `commands/character.rs` | 205 | 角色卡 12 個＋匯入收據 3 個＋`load_active_cards` |
| `commands/world.rs` | 189 | 桌 10 個＋世界書 10 個 |
| `commands/state.rs` | 155 | 狀態樹、分支綁定、`mechanism_ledger`、`BranchBinding` |
| `commands/genesis.rs` | 128 | 開桌生成大綱／角色／擴寫＋三個 Outcome DTO |
| `commands/settings.rs` | 102 | 設定、`usage_report`、`current_models`、CLI 模型目錄 |
| `lib.rs` | 157 | 25 行 mod 宣告＋`data_root` 7＋`config_root` 7＋`run()` 114 |

合計 3982（含各檔 import 標頭與測試模組外殼；拆前 lib.rs 為 3841）。

**`ai_transport.rs` 與 cli.rs 平級，不放 `commands/` 底下**：那 10 個項目沒有一個是 command，卻被 chat／refactor／scene／genesis／image／settings 六組呼叫。放進 commands/ 會讓六個 command 檔去 import 另一個 command 檔。依賴方向：`commands/* → ai_transport → cli / transport / lanes / usage_log`。

**`chat.rs` 維持單檔**：切成「角色回合／GM 回合」前者太碎、後者仍是最大檔。日後真要再拆，唯一自然的一刀是 `arrivals.rs`＝`load_hidden_cards` + `record_*_arrivals` + 對應測試。

## 跨檔呼叫（實測，共 11 處）

- `data_root`／`config_root` 被 11 組用 → **維持私有 `fn`**。crate root 的私有項對後代模組本來就可見（已用最小 crate 實測編譯通過）。
- `load_active_cards` 住 character、被 chat 與 state 用 → `pub(super)`。
- ai_transport 那組被六組用 → `pub(crate)`。
- `grok_profile` 隨 ai_transport 走，消掉 cli_setup 的反向依賴。

## Tauri 2 注意事項

- 跨模組路徑 `commands::chat::foo` 是官方支援方式；**command 名稱不含模組，全 crate 仍須唯一**。
- 本專案鎖定 tauri 2.11.5 / tauri-macros 2.6.3，對 `pub(crate)` command 也會匯出 `__cmd__`（官方文件只保證 `pub`）。**升級 Tauri 時要重驗這點。**
- 不可拆成多次 `.invoke_handler()`——後一次會取代前一次。維持單一 98 項清單。
- command 提升 `pub(crate)` 後，簽名裡的私有 DTO（`InstallMessages`、`CharacterImport`、`SceneAppearances`、`BranchBinding`、各 Outcome）要一併提升，否則只出 warning、測試不會紅。
- lib.rs 有三處平台條件碼（67 `#[cfg(unix)]`、244 `#[cfg(target_os="windows")]`、304 `#[cfg(unix)]`），全落在會搬去 `cli_setup.rs` 那批。

## 主風險

`generate_handler!` 清單漏抄一個名字，**Rust 照樣編譯通過**，該 command 只是沒註冊，要等前端 invoke 才 runtime 報 command not found。

## 驗收六項

1. **command 名 multiset 比對**（拆前／拆後）——不可先 `uniq`，數量、重複名、attribute、完整簽名一起核；`generate_handler!` 清單同樣比對。
2. **搬移項目文字等價**——只允許 visibility 與模組路徑差異。helper 漏搬、import 綁錯、參數改名都可能編譯成功。
3. **審每個新檔的 import map**，斷言 `ai_transport.rs` 不依賴 `crate::commands`。
4. **測試 leaf-name multiset** 拆前後比對，`cargo test` 530 全綠。
5. `RUSTFLAGS="-Dprivate_interfaces" cargo check` 綠。
6. **Windows CI**（`.github/workflows/ci-windows-verify.yml`）＋ 真機 invoke smoke：打 release 包開一桌跑一個 GM 回合，踩 chat／scene／state／ai_transport 四個新檔。

前端 61 個 ts/tsx 不動，不需檢查頁面偏移。98 個 command 前端全是寫死字串呼叫，無動態拼名，靜態比對覆蓋 100%。

## 開工順序

1. 抓拆前基準（command 名、`generate_handler!` 清單、測試 leaf-name），存檔
2. 建 `commands/` 骨架與 `ai_transport.rs`
3. 一個領域一個領域搬，每搬完一個跑一次 `cargo check`
4. 六項驗收
