# 測試通道截不到卡片介面 iframe

## Summary
測試通道 `shot` 拍卡片介面時 iframe 內容是整片底色。interface-scene-change 改用 `js` 取 iframe `srcdoc` 落檔、本機 http 伺服器在 Browser pane 渲染截圖繞過。2026-10-06 衍生。

## Next action
先把這條寫進 test-harness 已知限制與繞法；再評估通道能否直接輸出 iframe 截圖。
