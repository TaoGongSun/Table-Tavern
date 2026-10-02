# desktop-update-detect — 桌面版 App 內更新與回退

分支：`desktop-update-detect`（HEAD `34b24f7`，包 1、包 2 已在分支上）。包 3 的程式在工作區，**還沒 commit、沒 push**。驗收五點已修，Sol 複核後的 slot 也改完，等再審。

## 現況
- 設計已拍板，規格在 [plans/desktop-update-detect.md](../plans/desktop-update-detect.md)。Mac 只出 Apple Silicon。
- 包 1（發版管線）完成：`test-v0.2.0` 演練全綠（2026-10-01）。
- 包 2（格式版本與遷移）完成：Opus、Sol 驗收三方共識（2026-10-02）。
- 包 3（偵測與一鍵更新）程式已落地，並修完 Opus＋Sol 驗收的五點，以及 Sol 複核的 slot。`npm run verify` 全綠（2026-10-02：vitest 202、cargo test lib 651 passed、1 ignored）。ignore 那條用演練檔 `Table Tavern.app.tar.gz` 另跑過，`codesign --verify --strict` 通過。沒有跑 GUI、沒有打包、沒有連 GitHub。畫面沒畫（包 5）。版本庫回退沒做（包 4）。
- npm `@tauri-apps/api`、`@tauri-apps/cli` 是 `~2.12.1`，對上 Rust `tauri` 2.12.1。不要退回，也不要動其他 npm 套件。
- 待作者裁決：唯讀桌能不能刪（目前可以，見計畫「實作時補定的細節」）。與包 3 無關。
- 包 4、包 5 還沒有程式。包 2–5 首次公開時必須同版發出。

## 這輪自行決定（審的時候看）
- 略過鍵在重驗通過、開閘之前清。安裝失敗時前端再呼叫既有的 `read_config`，經 `onConfig` 寫回，不新增事件。命令仍回 `Result<(), String>`。
- 只有正在下載、正在安裝時 `update_check` 不覆寫、不打網路。已下載還沒安裝仍會連網：遠端仍是同一個版本就維持已下載，可直接裝；遠端是別的版本就改成已檢查，舊的版本庫目錄留著交包 4。遠端沒有更新時槽維持已下載，這次檢查仍回 None，不把舊版再推到畫面上。`update_download` 回版本字串，`update_install` 帶版本核對。新錯誤字串：「已在下載」「下載的版本與要安裝的版本不同」。
- tar 權限只留 `mode & 0o777`。相對連結解析後仍在目的目錄內才建立；硬連結與指向外部的連結拒絕，錯誤字串仍是「壓縮檔含連結」。
- 同一輪殘留整理若剛把 `.TableTavern-update.app` 改名成 previous，這一輪不刪那份 previous（下一輪 `update_post_launch` 才刪較舊的）。
- 在途、且沒握世界鎖的 AI／匯出不在安裝的等待集合裡，新呼叫在入口被拒。

## 下一步
- Opus、Sol 再審包 3。要修就在這條工作線上改，仍不 commit，直到三方共識。
