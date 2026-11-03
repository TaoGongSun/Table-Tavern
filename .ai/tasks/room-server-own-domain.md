# Task
Task-ID: room-server-own-domain
Title: 房間伺服器換成 Table Tavern 專用網域
Status: todo
Created: 2026-11-03

## Summary
房間伺服器目前掛在 `tt-room.pirahus.com`（作者另一個無關專案的網域），只是朋友測試期間暫用；測試順利後換成專用網域〔作者裁決 2026-11-03，不急〕。

## 要做的事
- 作者自己買網域（付款由作者做）。中國大陸有朋友就要用自訂網域，`workers.dev` 普遍被封。
- `party/server/workers/wrangler.toml` 的 routes 加新網域、重新部署；桌面版預設網址改新網域，跟下一次打包一起做。
- 過渡期兩個網域同時掛在同一個 Worker，舊版桌面版照常連；大家都換新版後再拿掉 `tt-room.pirahus.com`。

## 估計
小案（核心幾行，主要是部署與打包）。
