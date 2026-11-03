// 定稿挑選：目前候選大圖＋候選縮圖列；立繪來源、生成（再骰）、挑定、上傳定稿。精靈 ③ 與立繪分頁共用。
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import "./vn-sprites.css";
import { t } from "../../i18n";
import type { AppConfig } from "../../shared/contracts/backend-contracts";
import { CLI_LABELS, detectClis } from "../ai-connection/cli";
import { updateConfig } from "../settings/update-config";
import type { SpriteSlot } from "./useSpriteSlot";

/** 立繪來源（偏好 `vn_sprite_source`，後端沒設＝codex）：裝了的 Codex／Gemini CLI 加 API */
function SpriteSourceSelect() {
  const [sources, setSources] = useState(["codex", "api"]);
  const [source, setSource] = useState("codex");
  useEffect(() => {
    void invoke<AppConfig>("read_config").then((config) => setSource(String(config.preferences["vn_sprite_source"] || "codex")));
    void detectClis().then((clis) => setSources([...clis.map((cli) => cli.id).filter((id) => id === "codex" || id === "agy"), "api"]));
  }, []);
  return (
    <label className="vn-source">
      {t("vnSpriteSource")}
      <select
        value={source}
        onChange={(event) => {
          setSource(event.currentTarget.value);
          void updateConfig({ preferences: { vn_sprite_source: event.currentTarget.value } });
        }}
      >
        {[...new Set([source, ...sources])].map((id) => (
          <option key={id} value={id}>
            {id === "api" ? "API" : CLI_LABELS[id]}
          </option>
        ))}
      </select>
      {source !== "codex" && <span className="field-note">{t("vnSpriteMattingNote")}</span>}
    </label>
  );
}

export function SpriteBasePicker({ slot, name }: { slot: SpriteSlot; name: string }) {
  const input = useRef<HTMLInputElement>(null);
  const candidates = slot.state?.candidates ?? [];
  const shown = slot.selected ? slot.previews[slot.selected] : slot.state?.sprite?.body;
  const disabled = slot.busy !== null;
  return (
    <div className="vn-picker">
      <SpriteSourceSelect />
      <div className="vn-picker-actions">
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={() => void slot.generate()}>
          {t(candidates.length ? "vnSpriteReroll" : "vnSpriteGenerate")}
        </button>
        <button
          type="button"
          className="btn btn-sm btn-primary"
          disabled={disabled || !slot.selected}
          onClick={() => void slot.pick()}
        >
          {t("vnSpritePick")}
        </button>
        <button type="button" className="btn btn-sm" disabled={disabled} onClick={() => input.current?.click()}>
          {t("vnSpriteUploadBase")}
        </button>
        <input
          ref={input}
          type="file"
          accept="image/png,image/webp"
          hidden
          onChange={(event) => {
            const file = event.currentTarget.files?.[0];
            event.currentTarget.value = "";
            if (file) void slot.upload(file);
          }}
        />
      </div>
      <div className="vn-picker-main">
        {shown ? (
          <img className="vn-picker-image" src={shown} alt={name} />
        ) : (
          <div className="vn-picker-empty">{t("vnSpriteNone")}</div>
        )}
        {slot.busy && <span className="vn-picker-busy">{t(`vnSpriteBusy_${slot.busy}`)}</span>}
      </div>
      {candidates.length > 1 && (
        <div className="vn-picker-strip">
          {candidates.map((file) => (
            <button
              key={file}
              type="button"
              className="vn-picker-thumb"
              aria-pressed={file === slot.selected}
              onClick={() => slot.setSelected(file)}
            >
              <img src={slot.previews[file]} alt="" />
            </button>
          ))}
        </div>
      )}
      {slot.error && (
        <p className="field-warn" role="alert">
          {slot.error}
        </p>
      )}
    </div>
  );
}
