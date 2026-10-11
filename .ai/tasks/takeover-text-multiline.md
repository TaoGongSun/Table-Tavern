# 重構接管：行式格式正文槽只顯示第一段

Status: todo

## Summary
Transfur-safe 骨架 `mainText|{{本回合.正文}}`，正文有換行時第二段起不帶 `mainText|` 前綴，面板不顯示。
細節見 [interface-takeover-card-types](../plans/interface-takeover-card-types.md)「發現的程式問題」。
同類：RPGImmortal 狀態欄骨架行有縮排，多行的小地圖值填進去只有第一行帶縮排，原卡 YAML 解析報錯、畫面不顯示（[takeover-interface-conflict](../handoffs/archive/takeover-interface-conflict.md) 實測）。兩者根因都是多行值填進有前綴／縮排的佔位行，一起修；RPGImmortal 畫面驗收等本案。

## Next action
未排程。
