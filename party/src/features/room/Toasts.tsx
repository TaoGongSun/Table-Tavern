// 房主提醒的浮層（room-status-strip §2）：不佔版面，疊在故事區頂端，最多三條、直排不重疊，點了就收。
import type { HostToast } from "./host-alerts";

export function Toasts({
  toasts,
  dismiss,
}: {
  toasts: HostToast[];
  dismiss: (id: number) => void;
}) {
  return (
    <div className="host-toasts" role="status" data-testid="host-toasts">
      <div className="host-toast-stack">
        {toasts.map((toast) => (
          <button
            key={toast.id}
            type="button"
            className="host-toast"
            data-testid="host-toast"
            onClick={() => dismiss(toast.id)}
          >
            {toast.text}
          </button>
        ))}
      </div>
    </div>
  );
}
