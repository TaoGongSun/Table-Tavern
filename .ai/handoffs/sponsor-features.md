# Handoff: sponsor-features

## Current state
規格與優先序（作者頁 → 配色 → AI 生圖）已拍板（見 tasks/sponsor-features.md）。第一項「作者頁」已實作完成，npm build 綠，等使用者實測。

## Completed
- 設定視窗新增第三分頁「作者」：頭像（Tao-icon.png，圓形 6rem）＋ 作者名 ＋ 一句文案 ＋「☕ 請作者喝咖啡」鈕（openUrl 開系統瀏覽器）
- 分頁切換沿用 AI 分頁未儲存確認（switchTab 統一處理 appearance／author）
- i18n zh-TW／en 三鍵：authorTab、authorBlurb、sponsorBtn
- 頭像資產：Tao-icon.png（240×240）複製為 src/assets/tao-icon.png，Vite 打包

## Verification
- `npm run build` exit 0（tsc＋vite，tao-icon 進 dist/assets）
- 程式碼：src/App.tsx（KOFI_URL 常數＋SettingsWindow author 分頁）、src/i18n.ts、src/App.css（.author-page／.author-avatar／.author-blurb）

## Remaining
- Ko-fi 連結為占位網址 `https://ko-fi.com/taogongsun`（App.tsx 頂部 KOFI_URL 常數），release-3-kofi 開帳後換正式網址
- 第二項：配色 +5（☕ 標記＋點擊即預覽、關設定視窗復原）——依賴 release-4 主題引擎拍板檔案格式
- 第三項：AI 生成角色圖（免費 3 次含提示詞）——動工前先查證 image API 接法
- 「匯入贊助包」入口未做（等贊助包檔案格式定案，預計也放作者頁）

## Next action
使用者實測作者頁後，續作配色 +5。

## Constraints
- 生圖模型接法動工前需查證拍板。
