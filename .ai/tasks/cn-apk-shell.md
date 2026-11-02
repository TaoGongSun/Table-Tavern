# Task
Task-ID: cn-apk-shell
Title: 網頁版包成 Android APK（中國安卓路線階段 2）
Status: todo
Created: 2026-11-02
案別：大案，核心估約 350 行（立案估）。會碰到解除安裝導致的存檔遺失，依規約要送方案審查。

## Summary
目標是讓中國玩家在安卓手機上完全用手機玩，接中國的模型〔作者裁決 2026-11-02〕。做法：用 Capacitor 把 `web/` 包成 APK，不移植桌面版。階段 1 [cn-web-provider](../handoffs/archive/cn-web-provider.md) 已經做完供應商的部分。

## 範圍
- Capacitor 殼：Android 專案、Gradle 打包。
- 安全標頭：APK 裡沒有 Cloudflare 的 `_headers`，要搬進頁面或殼的設定。頁面裡的 meta 不認 `frame-ancestors` 和 `sandbox` 兩條指令，所以卡片沙盒 iframe 與 zod 隔離頁要重新驗證。
- 簽章：要產生 keystore。keystore 遺失就再也發不出同簽章的更新，所以第一版就要離線備份，再放進 CI secrets。
- 版本檢查：Android 不准 App 自己安裝更新。做法是開 App 時檢查有沒有新版，有就開下載頁給玩家。檢查網址不能放 GitHub，因為 GitHub 在中國不穩。
- 存檔：解除安裝就會清掉存檔。網頁版已經有匯出存檔，APK 要讓玩家知道這件事，並確認在手機上匯出、匯入都走得通。
- 中國供應商模式下，頂欄改成「下載 APK」。

## 已知限制與前置
- 作者沒有中國品牌手機實機，只能用 Android 模擬器驗證；中國 ROM 的安裝攔截、廠商改過的 WebView 都測不到。〔作者 2026-11-02 告知〕
- 不辦 ICP，所以上不了中國的應用商店，只能讓玩家側載。華為純血鴻蒙裝不了 APK，那些玩家只能走網頁。
- CI 打包要等作者說了才觸發。
- 下載頁要放在中國連得上的地方，這部分歸階段 3「發佈」，需要先買自有網域。
- 主力入口（網頁或 APK）等託管結果再定〔作者裁決 2026-11-02〕。
