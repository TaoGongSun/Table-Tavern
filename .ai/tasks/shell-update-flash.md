# Task
Task-ID: shell-update-flash
Title: 卡片介面殼更新無閃白：postMessage ready 取代 load 事件重建雙緩衝
Status: todo
Created: 2026-08-13T00:30:01.705137+08:00
Updated: 2026-08-13T00:30:01.705137+08:00

## Summary
2026-08-12 立案（使用者裁決）。卡片介面原本的雙格＋onLoad 翻面雙緩衝在 WKWebView 上不可靠——水印實證：帶 script 的 srcdoc iframe 的 load 事件不觸發、塞格 setState 也無聲失效，三次「卡片介面全空白」事故（結果視窗、全桌介面、毛絨轉變桌）皆源於此，已整台拆除改單 iframe 直繪（commit 見 refactor-survey-spans 當日）。代價：殼值更新（狀態樹變＝每回合 GM 回覆後）時 iframe 換 key 重掛，可能閃一下白。ST 不閃白是因為它每則訊息各自一個 iframe 排在訊息流、舊的留著，從頭到尾沒有「翻面」動作，不依賴載入訊號；我們是單一覆蓋層雙格疊放，翻面必須有「載好了」訊號，而 WKWebView 的 load 事件靠不住。

## Next action
- 2026-08-12 立案；現行單 iframe 直繪是正確基線，開工前先實測閃白痛感（每回合一次、毫秒級）再定優先序

## Constraints
- 單 iframe 直繪是現行正確基線：任何重建必須先過「毛絨轉變桌開介面、開關多次、跨桌切換」三步實測才准替換。
- 殼 sandbox 維持 allow-scripts（無 allow-same-origin），ready 訊號只能走 postMessage。
