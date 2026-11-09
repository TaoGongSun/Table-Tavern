import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../../i18n";
import type { AppConfig } from "../../../shared/contracts/backend-contracts";
import { CLI_LABELS, cliConnectedKey, detectClis } from "../../ai-connection/cli";
import { updateConfig } from "../../settings/update-config";

export async function imageSources() {
  const [config, clis] = await Promise.all([invoke<AppConfig>("read_config"), detectClis()]);
  const sources = clis.filter((cli) => ["codex", "agy"].includes(cli.id) && !cli.update_state && config.preferences[cliConnectedKey(cli.id)] === true).map((cli) => cli.id);
  if (config.api_keys?.openrouter?.trim()) sources.push("api");
  const saved = String(config.preferences.vn_sprite_source ?? "");
  return { sources, source: sources.includes(saved) ? saved : (sources[0] ?? "") };
}

export function SpriteSourceSelect({ onChange, onSelect, disabled = false }: { onChange?: (source: string) => void; onSelect?: () => void; disabled?: boolean }) {
  const [choice, setChoice] = useState({ sources: [] as string[], source: "" });
  const [error, setError] = useState(false);
  useEffect(() => {
    let live = true;
    imageSources().then(async (next) => { if (next.source) await updateConfig({ preferences: { vn_sprite_source: next.source } }); if (live) { setChoice(next); onChange?.(next.source); } }).catch(() => { if (live) setError(true); });
    return () => { live = false; };
  }, [onChange]);
  return <label className="vn-source">{t("vnSpriteSource")}
    <select disabled={disabled || !choice.sources.length} value={choice.source} onChange={(event) => {
      const source = event.currentTarget.value;
      setError(false);
      void updateConfig({ preferences: { vn_sprite_source: source } }).then(() => {
        setChoice((current) => ({ ...current, source })); onChange?.(source); onSelect?.();
      }).catch(() => setError(true));
    }}>{choice.sources.map((id) => <option key={id} value={id}>{CLI_LABELS[id] ?? "API"}</option>)}</select>
    {(!choice.source || error) && <span className="field-note">{t(error ? "vnArtActionFailed" : "vnArtNoSource")}</span>}
  </label>;
}
