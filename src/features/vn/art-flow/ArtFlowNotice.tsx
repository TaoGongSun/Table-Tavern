import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { t } from "../../../i18n";
import type { ArtStatus } from "../../../shared/contracts/backend-contracts";
import { updateConfig } from "../../settings/update-config";
import { onSpriteStatus } from "../useSprites";
import { BACKGROUND_STATUS_EVENT } from "../useBackgrounds";
import { listen } from "@tauri-apps/api/event";
import { SpriteSourceSelect } from "./SpriteSourceSelect";

export function ArtFlowNotice({ worldId }: { worldId: string }) {
  const [status, setStatus] = useState<ArtStatus | null>(null);
  const [source, setSource] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [batch, setBatch] = useState<number | null>(null);
  const read = useCallback(() => invoke<ArtStatus>("vn_art_status", { worldId }).then(setStatus), [worldId]);
  useEffect(() => {
    let live = true;
    const refresh = () => { if (live) void read().catch(() => {}); };
    refresh();
    const off = onSpriteStatus(worldId, refresh);
    const bg = listen<{ worldId: string }>(BACKGROUND_STATUS_EVENT, ({ payload }) => { if (payload.worldId === worldId) refresh(); });
    return () => { live = false; off(); void bg.then((stop) => stop()); };
  }, [read, worldId]);
  const remaining = status ? status.total - status.ready + status.backgrounds : 0;
  return <div className="vn-art-flow">
    <SpriteSourceSelect onChange={setSource} disabled={busy} />
    <p>{status?.art.started ? t("vnArtWorking", { n: Math.max(0, (batch ?? remaining) - remaining), total: batch ?? remaining }) : t("vnArtNotStarted")}</p>
    <p className="field-note">{t("vnArtEstimate", { source: source === "agy" ? "Gemini CLI" : source === "api" ? "API" : "Codex CLI", n: status ? status.total - status.ready : 0, bg: status?.backgrounds ?? 0, faces: status?.faces ?? 0 })}</p>
    {!status?.art.started && <button type="button" className="btn btn-primary" disabled={!source || busy} onClick={() => {
      setBusy(true); setError(false); setBatch(remaining);
      void updateConfig({ preferences: { vn_sprite_source: source } }).then(() => invoke("vn_art_start", { worldId })).then(read).catch(() => setError(true)).finally(() => setBusy(false));
    }}>{t("vnArtBegin")}</button>}
    {error && <p className="field-warn">{t("vnArtActionFailed")}</p>}
  </div>;
}
