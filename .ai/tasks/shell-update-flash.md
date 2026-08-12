# Task
Task-ID: shell-update-flash
Title: 卡片介面殼更新無閃白：postMessage ready 取代 load 事件重建雙緩衝
Created: 2026-08-12T17:05:00+08:00
Status: todo

## Summary
2026-08-12 立案（使用者裁決）。卡片介面原本的雙格＋onLoad 翻面雙緩衝在 WKWebView 上不可靠——水印實證：帶 script 的 srcdoc iframe 的 load 事件不觸發、塞格 setState 也無聲失效，三次「卡片介面全空白」事故（結果視窗、全桌介面、毛絨轉變桌）皆源於此，已整台拆除改單 iframe 直繪（commit 見 refactor-survey-spans 當日）。代價：殼值更新（狀態樹變＝每回合 GM 回覆後）時 iframe 換 key 重掛，可能閃一下白。ST 不閃白是因為它每則訊息各自一個 iframe 排在訊息流、舊的留著，從頭到尾沒有「翻面」動作，不依賴載入訊號；我們是單一覆蓋層雙格疊放，翻面必須有「載好了」訊號，而 WKWebView 的 load 事件靠不住。

## Next action
重建雙緩衝時改用自有訊號：buildShellDocument 包裝層注入「DOMContentLoaded 時 `postMessage({source:'table-tavern-card', kind:'ready'})`」，父頁收到 ready 才翻面——postMessage 通道已實證可用（殼按鈕送輸入走同一條）。重建時翻面狀態用兩個獨立 state（backShell/frontShell）而非 tuple＋slot 指標，避開這次 setState 無聲失效的形狀。開工前先實測閃白的實際痛感（頻率＝每回合一次、時長毫秒級）再定優先序。

## Constraints
- 單 iframe 直繪是現行正確基線：任何重建必須先過「毛絨轉變桌開介面、開關多次、跨桌切換」三步實測才准替換。
- 殼 sandbox 維持 allow-scripts（無 allow-same-origin），ready 訊號只能走 postMessage。
