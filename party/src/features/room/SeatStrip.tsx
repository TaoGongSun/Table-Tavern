// 桌邊狀態列（room-status-strip §1）：故事頁回合列下方，誰在打字、誰送了、誰離線、誰交給 AI。
// 房主點別人開選單（催一下、代管）；沒有可做的事（大廳、點自己、非房主）就是普通文字，不開空選單。
import { useEffect, useState } from "react";
import { t } from "../../i18n";
import { PartyMenu, type PartyMenuItem } from "../controls/PartyMenu";
import { useDelegate } from "./DelegateButton";
import { canDelegate } from "./delegation";
import type { RoomView } from "./room-state";
import type { Act } from "./useRoom";

type Person = RoomView["members"][number];

type BadgeKind = "off" | "delegated" | "drafting" | "submitted" | "skipped" | "next" | "unseated";

const BADGE_KEY = {
  off: "offline",
  delegated: "seatDelegated",
  submitted: "seatSubmitted",
  skipped: "statusSkipped",
  next: "seatNext",
  unseated: "seatUnseated",
} as const;

function badgeFor(view: RoomView, m: Person): { kind: BadgeKind | null; dim: boolean } {
  if (!m.connected) return { kind: "off", dim: true };
  if (m.delegated) return { kind: "delegated", dim: false };
  const round = view.round;
  if (round?.phase === "acting") {
    const entry = round.roster.find((e) => e.memberId === m.memberId);
    if (entry?.status === "drafting") return { kind: "drafting", dim: false };
    if (entry?.status === "submitted") return { kind: "submitted", dim: false };
    if (entry?.status === "skipped") return { kind: "skipped", dim: false };
    if (entry) return { kind: null, dim: false };
    if (m.cardId) return { kind: "next", dim: true };
    return { kind: null, dim: false };
  }
  const carried = round?.carry?.find((e) => e.memberId === m.memberId);
  if (carried?.status === "submitted") return { kind: "submitted", dim: false };
  if (carried?.status === "skipped") return { kind: "skipped", dim: false };
  if (view.room.status === "lobby" && !m.cardId) return { kind: "unseated", dim: true };
  return { kind: null, dim: false };
}

/** 有名單就照名單，其餘入座者接在後面；大廳是入座者在前、還沒選角的在後。旁觀者不進開演中的列。 */
function people(view: RoomView): Person[] {
  const seated = view.members.filter((m) => m.cardId);
  if (view.room.status === "lobby") return [...seated, ...view.members.filter((m) => !m.cardId)];
  if (view.room.status !== "playing" && view.room.status !== "transition") return [];
  const order = view.round?.roster.map((e) => e.memberId) ?? [];
  const head = order
    .map((id) => seated.find((m) => m.memberId === id))
    .filter((m): m is Person => !!m);
  const seen = new Set(head.map((m) => m.memberId));
  return [...head, ...seated.filter((m) => !seen.has(m.memberId))];
}

function useNudge(act: Act) {
  const [until, setUntil] = useState<Record<string, number>>({});
  const [now, setNow] = useState(0);
  useEffect(() => {
    const times = Object.values(until).filter((at) => at > Date.now());
    if (!times.length) return;
    const id = setTimeout(() => setNow(Date.now()), Math.max(0, Math.min(...times) - Date.now()));
    return () => clearTimeout(id);
  }, [until, now]);
  const cooled = (id: string) => (until[id] ?? 0) > Date.now();
  const go = async (round: number, memberId: string) => {
    const reply = await act("host.nudge", { round, memberId });
    const wait = reply.ok
      ? 30_000
      : reply.code === "E_RATE_LIMITED"
        ? (reply.retryAfterMs ?? 30_000)
        : 0;
    if (wait) setUntil((u) => ({ ...u, [memberId]: Date.now() + wait }));
  };
  return { cooled, go };
}

function Avatar({ name, card }: { name: string; card: RoomView["cards"][number] | null }) {
  const style = card?.color ? { borderColor: card.color, color: card.color } : undefined;
  if (card?.avatar) return <img className="seat-avatar" src={card.avatar} alt="" style={style} />;
  return (
    <span className="seat-avatar" style={style} aria-hidden="true">
      {Array.from(name)[0] ?? ""}
    </span>
  );
}

function Badge({ kind }: { kind: BadgeKind }) {
  if (kind === "drafting") {
    return (
      <span className="seat-badge">
        {t("statusDrafting")}
        <span className="seat-dots" aria-hidden="true">
          <i />
          <i />
          <i />
        </span>
      </span>
    );
  }
  return <span className="seat-badge">{t(BADGE_KEY[kind])}</span>;
}

export function SeatStrip({ view, act }: { view: RoomView; act: Act }) {
  const nudge = useNudge(act);
  const delegate = useDelegate(view, act);
  if (view.room.status === "closed") return null;
  const rows = people(view);
  if (!rows.length) return null;
  const host = view.room.hostId === view.me.memberId;
  const round = view.round;
  const itemsFor = (m: Person): PartyMenuItem[] => {
    const items: PartyMenuItem[] = [];
    const entry =
      round?.phase === "acting" ? round.roster.find((e) => e.memberId === m.memberId) : undefined;
    const waiting = entry?.status === "pending" || entry?.status === "drafting";
    if (waiting && m.connected && round) {
      const cooled = nudge.cooled(m.memberId);
      items.push({
        key: "nudge",
        label: t(cooled ? "nudgeDone" : "nudge"),
        disabled: cooled,
        // 選單元件的項目沒有 testid 欄位，掛在項目裡才點得到
        icon: <span data-testid={`nudge-${m.memberId}`} />,
        onSelect: () => void nudge.go(round.round, m.memberId),
      });
    }
    if (canDelegate(view)) {
      const on = !m.delegated;
      items.push({
        key: "delegate",
        label: t(on ? "delegateOnOther" : "delegateOff"),
        onSelect: () => delegate.start([m.memberId], on, t(on ? "delegateOnOther" : "delegateOff")),
      });
    }
    return items;
  };
  return (
    <>
      <div className="seat-strip" data-testid="seat-strip">
        {rows.map((m) => {
          const card = m.cardId ? (view.cards.find((c) => c.cardId === m.cardId) ?? null) : null;
          const name = card ? card.name : m.nickname;
          const title = card
            ? t("seatChipTitle", { name: m.nickname, card: card.name })
            : m.nickname;
          const { kind, dim } = badgeFor(view, m);
          const face = (
            <>
              <Avatar name={name} card={card} />
              <span className="seat-name">{name}</span>
              {kind && <Badge kind={kind} />}
            </>
          );
          const items = host && m.memberId !== view.me.memberId ? itemsFor(m) : [];
          return (
            <span key={m.memberId} className="seat-slot" style={dim ? { opacity: 0.5 } : undefined}>
              {items.length > 0 ? (
                <PartyMenu
                  label={title}
                  ariaLabelTrigger={false}
                  className="seat-chip"
                  trigger={
                    <span className="seat-face" data-testid={`seat-${m.memberId}`}>
                      {face}
                    </span>
                  }
                  items={items}
                />
              ) : (
                <span className="seat-chip" data-testid={`seat-${m.memberId}`} title={title}>
                  {face}
                </span>
              )}
            </span>
          );
        })}
      </div>
      {delegate.confirm}
      {delegate.error && <div className="field-error">{delegate.error}</div>}
    </>
  );
}
