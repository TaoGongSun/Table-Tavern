import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { t } from "../../../i18n";
import { updateConfig } from "../../settings/update-config";
import { useArtStatus } from "./useArtStatus";
import { useQuotaRecovery } from "./useQuotaRecovery";
import { SpriteSourceSelect } from "./SpriteSourceSelect";

export function ArtFlowNotice({ worldId, onStarted, onPause, compact = false }: { worldId: string; onStarted?: (started: boolean) => void; onPause?: (paused: boolean) => void; compact?: boolean }) {
  const { status, read } = useArtStatus(worldId);
  const [source, setSource] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const [batch, setBatch] = useState<number | null>(null);
  const retry = () => {
    setBusy(true); setError(false);
    void invoke("vn_art_retry", { worldId }).then(read).catch(() => setError(true)).finally(() => setBusy(false));
  };
  useEffect(() => { if (status) { onStarted?.(status.art.started); onPause?.(Boolean(status.pause)); } }, [status, onStarted, onPause]);
  const remaining = status ? status.total - status.ready + status.backgrounds : 0;
  const recovery = useQuotaRecovery(status?.pause?.reset ?? null);
  const start = <button type="button" className="btn btn-primary" disabled={!status || !source || busy} onClick={() => {
    setBusy(true); setError(false); setBatch(remaining);
    void updateConfig({ preferences: { vn_sprite_source: source } }).then(() => invoke("vn_art_start", { worldId })).then(read).catch(() => setError(true)).finally(() => setBusy(false));
  }}>{t("vnArtBegin")}</button>;
  const retryButton = <button type="button" className="btn btn-sm" disabled={busy} onClick={retry}>{t("vnArtRetryNow")}</button>;
  if (compact && (!status || (!status.pause && !status.failed && status.ready >= status.total))) return null;
  if (compact) return <div className="vn-art-summary">
    <span>{status!.pause ? t("vnArtQuotaShort", { time: recovery }) : status!.failed ? t("vnArtFailed", { n: status!.failed }) : status!.art.started ? t("vnArtPreparing", { n: status!.ready, total: status!.total }) : t("vnArtNoSprites", { panel: t("vnGallery") })}</span>
    {(status?.pause || Boolean(status?.failed)) && retryButton}
    {error && <span className="field-warn">{t("vnArtActionFailed")}</span>}
  </div>;
  const expressions = status?.facesPerCharacter != null
    ? t("vnArtExpressionsEach", { each: status.facesPerCharacter })
    : t("vnArtExpressionsTotal", { n: status?.faces ?? 0 });
  return <div className="vn-art-flow">
    <div className="vn-art-controls">
      <SpriteSourceSelect onChange={setSource} onSelect={status?.art.started ? retry : undefined} disabled={busy} />
      {!status?.art.started && start}
      {status?.art.started && retryButton}
    </div>
    <p>{status?.pause ? t("vnArtStopped", { n: batch === null ? status.ready : Math.max(0, batch - remaining), total: batch ?? status.total + status.backgrounds }) : status?.art.started ? remaining === 0 && status.pending === 0 ? t("vnArtReady") : batch === null ? t("vnArtPreparing", { n: status.ready, total: status.total }) : t("vnArtWorking", { n: Math.max(0, batch - remaining), total: batch }) : t("vnArtNotStarted")}</p>
    {source && <p className="field-note">{t("vnArtEstimate", { source: source === "agy" ? "Gemini CLI" : source === "api" ? "API" : "Codex CLI", n: status ? status.total - status.ready : 0, bg: status?.backgrounds ?? 0 })}</p>}
    {source && <p className="field-note">{expressions}</p>}
    {Boolean(status?.failed) && <p className="field-warn">{t("vnArtFailed", { n: status?.failed ?? 0 })}</p>}
    {status?.pause && <p className="field-warn">{status.pause.reset !== null ? t("vnArtPaused", { n: remaining, time: recovery }) : t("vnArtPausedUnknown", { n: remaining })}</p>}
    {error && <p className="field-warn">{t("vnArtActionFailed")}</p>}
  </div>;
}
