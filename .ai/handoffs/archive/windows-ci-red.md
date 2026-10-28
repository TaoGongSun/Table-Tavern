# windows-ci-red — Windows verify CI 自 2026-10-03 起一直紅

已結案：Windows CI（run 40683798025，含一次性 release exe 清單核對）與本機 verify 皆綠，Sol、opus-review 驗收 PASS。

## 定案做法
- A 測試編譯：`fake_claude` 限 unix，`opening_raw::sent_prompt`、`owner::impl Table`、`owner::run_knight`、`reminder_matching_…` 後半同步限 unix。
- B 0xc0000139：tauri-build 的資訊清單（Common Controls v6）經 tauri-winres→embed-resource 以 `rustc-link-arg-bins` 只連進 app 執行檔，測試程式沒有。改 `WindowsAttributes::new_without_app_manifest()`＋build.rs 以 `rustc-link-arg=/MANIFEST:EMBED /MANIFESTINPUT:<abs>` 嵌同一份（同 tauri 本身 `embed_manifest_for_tests`），清單檔 `src-tauri/windows-app-manifest.xml`；不加 `/MANIFESTUAC:NO`（連結器預設 asInvoker）。release.yml 打包後以 `scripts/release/check-windows-manifest.ps1` 核對：Common-Controls 6.0.0.0 恰好一份、asInvoker、uiAccess=false。
- C 換行：`.gitattributes` 全庫 `* text=auto eol=lf`＋圖檔 binary，已 `git add --renormalize .`（opencc 兩個授權檔 CRLF→LF）。**既有 Windows 工作副本拉下來後 git status 會一次性出現大量變動**，照 `git add --renormalize .` 或重新取出即可。
- D grok 抹寫在 Windows 一律失敗（根因：`scan_residue` 讀到自己正持獨占鎖的 `.lock`，Windows 強制鎖讀取回 os error 33）：空的一般檔（`is_file && len==0`、非 .json）不開檔；空 .json 照判解析失敗；不依 `.lock` 副檔名特判。

## 已知限制
- D 不是一致性快照：metadata 只代表取樣當下為空。
- grok_session 寫回用 `fs::rename`，Windows 上目標被別的程序開著可能撞 os error 5／32〔模型判斷·未裁決〕。
- 123 個限 unix 的測試（假 CLI 是 shell 腳本）Windows 跑不到，由主線另立案。
