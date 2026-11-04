// 邀請與交給手機的連結轉 QR 碼（手機房主頁與桌面資訊列共用）：uqr 只產矩陣，這裡自己畫成 SVG（不走 innerHTML）。
import { encode } from "uqr";

export function QrCode({ value, label }: { value: string; label: string }) {
  const { size, data } = encode(value, { border: 2 });
  const cells: string[] = [];
  data.forEach((row, y) => row.forEach((on, x) => on && cells.push(`M${x} ${y}h1v1h-1z`)));
  return (
    <svg
      className="room-qr"
      viewBox={`0 0 ${size} ${size}`}
      role="img"
      aria-label={label}
      shapeRendering="crispEdges"
    >
      <rect width={size} height={size} fill="#fff" />
      <path d={cells.join("")} fill="#000" />
    </svg>
  );
}
