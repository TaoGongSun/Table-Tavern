# 匯入卡的世界書角色自己看不到

Status: todo〔作者裁決 2026-10-07：立案，列為網頁版公開前門檻〕

## Summary
匯入角色卡時，卡內 `character_book` 沒帶 `extensions.table_tavern.visibility` 的條目一律設成 GM 可見（`src-tauri/src/data/worldbook.rs:718`），角色線組提示時把 GM 條目濾掉（`src-tauri/src/transport/turns.rs:160`）。結果是原卡直玩的角色看不到自己卡裡的設定，與 SillyTavern（卡的世界書就是給這個角色用的）不同。web-version 第 1 輪審查（Grok）發現。

做法方向：卡內世界書（角色卡路匯入併進桌的那批）預設改為該角色可見；世界書路匯入（`import/card.rs:458`，沒有角色 ID）與 GM 專用條目怎麼區分，開工時定。已存桌的既有條目要不要補救，照 memory「舊桌不相容（限發佈前）」判。

## Next action
未排程。開工先列出三條匯入路（角色卡路、世界書路、重構）各自怎麼寫 visibility，再決定預設值改在哪一層；補一條測試：匯入帶 character_book 的卡後，角色線下一輪送出的 messages 含該卡的 constant 條目。
