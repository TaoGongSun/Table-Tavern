import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { t } from "../../../i18n";
import type { ArtStatus } from "../../../shared/contracts/backend-contracts";
import { updateConfig } from "../../settings/update-config";
import { onSpriteStatus } from "../useSprites";
import { BACKGROUND_STATUS_EVENT } from "../useBackgrounds";
import { listen } from "@tauri-apps/api/event";
import { SpriteSourceSelect } from "./SpriteSourceSelect";

export function ArtFlowNotice({ worldId, onStarted }: { worldId: string; onStarted?: (started: boolean) => void }) {
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
    const flow = listen("vn-art-flow-status", refresh);
    const bg = listen<{ worldId: string }>(BACKGROUND_STATUS_EVENT, ({ payload }) => { if (payload.worldId === worldId) refresh(); });
    return () => { live = false; off(); void flow.then((stop) => stop()); void bg.then((stop) => stop()); };
  }, [read, worldId]);
  const retry = () => {
    setBusy(true); setError(false);
    void invoke("vn_art_retry", { worldId }).then(read).catch(() => setError(true)).finally(() => setBusy(false));
  };
  useEffect(() => { if (status) onStarted?.(status.art.started); }, [status, onStarted]);
  const remaining = status ? status.total - status.ready + status.backgrounds : 0;
  return <div className="vn-art-flow">
    <SpriteSourceSelect onChange={setSource} onSelect={status?.art.started ? retry : undefined} disabled={busy} />
    <p>{status?.art.started ? remaining === 0 && status.pending === 0 ? t("vnArtReady") : batch === null ? t("vnArtPreparing", { n: status.ready, total: status.total }) : t("vnArtWorking", { n: Math.max(0, batch - remaining), total: batch }) : t("vnArtNotStarted")}</p>
    {source && <p className="field-note">{t("vnArtEstimate", { source: source === "agy" ? "Gemini CLI" : source === "api" ? "API" : "Codex CLI", n: status ? status.total - status.ready : 0, bg: status?.backgrounds ?? 0, faces: status?.faces ?? 0 })}</p>}
    {Boolean(status?.failed) && <p className="field-warn">{t("vnArtFailed", { n: status?.failed ?? 0 })}</p>}
    {status?.pause && <p className="field-warn">{t(status.pause.reset ? "vnArtPaused" : "vnArtPausedUnknown", { n: status.pending, time: status.pause.reset ? new Date(status.pause.reset).toLocaleString() : "" })}</p>}
    {status?.art.started && <button type="button" className="btn btn-sm" disabled={busy} onClick={retry}>{t("vnArtRetryNow")}</button>}
    {!status?.art.started && <button type="button" className="btn btn-primary" disabled={!status || !source || busy} onClick={() => {
      setBusy(true); setError(false); setBatch(remaining);
      void updateConfig({ preferences: { vn_sprite_source: source } }).then(() => invoke("vn_art_start", { worldId })).then(read).catch(() => setError(true)).finally(() => setBusy(false));
    }}>{t("vnArtBegin")}</button>}
    {error && <p className="field-warn">{t("vnArtActionFailed")}</p>}
  </div>;
}
