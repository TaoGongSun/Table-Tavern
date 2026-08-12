# 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool）

本檔存放 [release-1-mac-signing](../tasks/release-1-mac-signing.md) 的規格細節（拍板結論、分包、驗收等），由任務檔的 Summary 連回。

## 已鋪好的前置（mvp-7 2026-07-22）
- `src-tauri/tauri.conf.json` 已有 `bundle.macOS.signingIdentity: "-"`（正式 codesign 蓋 ad-hoc，含 hardened runtime、Info.plist 綁定、資源封裝）。憑證到手後把該值改為 `"Developer ID Application: <名稱> (<TEAMID>)"` 即可，其餘結構不用動。
- 驗收指令：`codesign -dvvv <app>` 看 flags／Info.plist／Sealed Resources；`spctl -a -vv <app>` 公證後應由 rejected 轉為 accepted。
- 重現下載情境測 Gatekeeper：`xattr -w com.apple.quarantine "0081;$(printf %x $(date +%s));Safari;" <dmg>`（測完 `xattr -d` 清掉），比 AirDrop 精準可重複。
- 公證通過後刪掉 README「第一次開啟被 Gatekeeper 擋下？」整段。
