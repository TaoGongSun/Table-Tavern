# Task handoff
Task-ID: test-build-cross-platform
Updated: 2026-07-30T21:40:00+08:00
Status: in_progress

## Goal
出一版可獨立運行的測試版：Mac DMG（ad-hoc 簽章）＋Windows 安裝檔（GitHub Actions 出未簽章 .msi/.exe）。正式簽章公證歸 release-1。

## Current state
產線可用，每次下令即可重打。最新一輪 0.2.0（2026-07-30 22:50，HEAD 1cc4ea7，含世界書卡匯入、條目就地展開編輯與未儲存確認）：Mac `src-tauri/target/release/bundle/dmg/Table Tavern_0.2.0_aarch64.dmg`（4.7MB，codesign `adhoc,runtime`，`--verify --deep --strict` 通過）＋Windows [run 30553621076](https://github.com/TaoGongSun/Table-Tavern/actions/runs/30553621076) success、artifact `table-tavern-windows-unsigned`（8.3MB，含 NSIS setup.exe 與 x64_en-US.msi）。剩使用者實機驗收：Mac DMG 拷去 MacBook Air 測 Gatekeeper、Windows artifact 在真 Windows 機安裝。

打包踩雷：Mac 端 `bundle_dmg.sh` 失敗時先看 `/Volumes/dmg.*` 有沒有上一輪殘留的暫存掛載卷，`hdiutil detach` 卸掉再重打即過（2026-07-28 實例）。

## Completed
- Mac：`npm run tauri build` 成功（rc=0），產出 `src-tauri/target/release/bundle/dmg/Table Tavern_0.1.0_aarch64.dmg`（4.5MB）
- Mac 驗證：codesign `Signature=adhoc`、`flags=0x10002(adhoc,runtime)`（非 mvp-7 出事的 linker-signed）；DMG 拷到乾淨路徑 hdiutil 掛載成功、`codesign --verify --deep --strict` OK；.app 啟動 5 秒存活、AppleScript quit 正常
- Windows：新增 `.github/workflows/test-build.yml`（workflow_dispatch＋`test-v*` tag 觸發；windows-latest、tauri-action、artifact 上傳），commit 9cb2a37 已 push
- CI 實跑成功：[run 30072316789](https://github.com/TaoGongSun/Table-Tavern/actions/runs/30072316789) conclusion=success，artifact `table-tavern-windows-unsigned`（7.3MB）含 `Table Tavern_0.1.0_x64_en-US.msi`（4.5MB）＋ `Table Tavern_0.1.0_x64-setup.exe`（NSIS，3.1MB）

## Verification
- build log 末段「Finished 2 bundles」（scratchpad tauri-build.log）
- codesign 輸出 `flags=0x10002(adhoc,runtime)`、verify「valid on disk」
- `gh run view 30072316789 --json conclusion` → success；`gh run download` 後 ls 實見 .msi 與 .exe

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 9cb2a37

## Remaining
- 使用者：把 DMG（路徑見上）拷去 MacBook Air 實測——期望顯示「Apple 無法驗證…」走系統設定「仍要打開」，不得再出現「已損毀」
- 使用者或協力者：從 GitHub Actions 該 run 頁下載 artifact，在真 Windows 機安裝驗收（SmartScreen 會警告「未知發行者」，測試版預期內；點「其他資訊→仍要執行」）
- 兩邊驗收結果回報後結案；若 Mac 端出「已損毀」需回頭查簽章

## Next action
- 等使用者實機驗收回報。無程式待辦。

## Constraints
- 不動 release-1（正式簽章公證）範圍；CI 不需 secrets（未簽章）
- artifact 在私有 repo 內，下載需登入 GitHub；保留期預設 90 天
