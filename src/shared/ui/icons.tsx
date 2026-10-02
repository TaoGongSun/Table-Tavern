// 全 app 共用的一組線條圖示：16 格 viewBox、筆畫吃 currentColor，跟著按鈕字色與七套主題換色。
// 自繪 inline SVG，不引外部圖示庫；圖示一律 aria-hidden，按鈕的名字由文字或 aria-label 提供。
import type { ReactNode } from "react";

interface IconProps {
  className?: string;
}

function Icon({ className, children }: IconProps & { children: ReactNode }) {
  return (
    <svg
      className={className ? `ico ${className}` : "ico"}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  );
}

export function IconBack(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M13.5 8h-9M8 4.5 4.5 8 8 11.5" />
    </Icon>
  );
}

export function IconMore(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="3.5" cy="8" r="1.1" fill="currentColor" />
      <circle cx="8" cy="8" r="1.1" fill="currentColor" />
      <circle cx="12.5" cy="8" r="1.1" fill="currentColor" />
    </Icon>
  );
}

export function IconPlus(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8 3v10M3 8h10" />
    </Icon>
  );
}

export function IconSettings(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="8" cy="8" r="2.2" />
      <path d="M8 1.8v1.7M8 12.5v1.7M1.8 8h1.7M12.5 8h1.7M3.6 3.6l1.2 1.2M11.2 11.2l1.2 1.2M3.6 12.4l1.2-1.2M11.2 4.8l1.2-1.2" />
    </Icon>
  );
}

export function IconCardInterface(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2" y="3" width="12" height="10" rx="1.5" />
      <path d="M2 6h12M6 6v7" />
    </Icon>
  );
}

export function IconSceneAdvance(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3 13V3l5 3 5-3v10" />
      <path d="M3 9.5 8 12l5-2.5" />
    </Icon>
  );
}

export function IconExport(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8 2.5v7.5M5 5.5 8 2.5l3 3" />
      <path d="M3 9.5v3a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-3" />
    </Icon>
  );
}

export function IconStop(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="4" y="4" width="8" height="8" rx="1.2" fill="currentColor" />
    </Icon>
  );
}

export function IconSend(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.5 8h9M8 4.5 11.5 8 8 11.5" />
    </Icon>
  );
}

export function IconEdit(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="m10.5 2.5 3 3L6 13H3v-3z" />
    </Icon>
  );
}

export function IconDelete(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.5 4.5h11M6.5 4.5V3h3v1.5" />
      <path d="M4 4.5l.7 8.6a1 1 0 0 0 1 .9h4.6a1 1 0 0 0 1-.9l.7-8.6M6.8 7v4.5M9.2 7v4.5" />
    </Icon>
  );
}

export function IconDrag(props: IconProps) {
  return (
    <Icon {...props}>
      <circle cx="6" cy="4" r="0.9" fill="currentColor" />
      <circle cx="10" cy="4" r="0.9" fill="currentColor" />
      <circle cx="6" cy="8" r="0.9" fill="currentColor" />
      <circle cx="10" cy="8" r="0.9" fill="currentColor" />
      <circle cx="6" cy="12" r="0.9" fill="currentColor" />
      <circle cx="10" cy="12" r="0.9" fill="currentColor" />
    </Icon>
  );
}

/** 向下的展開箭頭；收合時要朝右的地方由 CSS 轉 -90 度 */
export function IconExpand(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="m4 6 4 4 4-4" />
    </Icon>
  );
}

export function IconHome(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M2.5 7.5 8 2.8l5.5 4.7" />
      <path d="M4 6.8V13h8V6.8M6.8 13V9.5h2.4V13" />
    </Icon>
  );
}

/** 四向星芒：一句話開桌（AI 幫你擺好桌） */
export function IconSparkle(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M8 2v3M8 11v3M2 8h3M11 8h3M4 4l2 2M10 10l2 2M12 4l-2 2M6 10l-2 2" />
    </Icon>
  );
}

/** 收納盒：窄陣容欄底部的封存與暫離入口 */
export function IconArchive(props: IconProps) {
  return (
    <Icon {...props}>
      <rect x="2" y="3" width="12" height="3" rx="1" />
      <path d="M3 6v6.5a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V6M6.5 9h3" />
    </Icon>
  );
}

/** 書本：角色卡轉成世界書條目 */
export function IconBook(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="M3 2.5h7.5a2 2 0 0 1 2 2v9H5a2 2 0 0 1-2-2z" />
      <path d="M3 11.5a2 2 0 0 1 2-2h7.5" />
    </Icon>
  );
}

/** 叉：對話窗的關閉鈕 */
export function IconClose(props: IconProps) {
  return (
    <Icon {...props}>
      <path d="m4 4 8 8M12 4l-8 8" />
    </Icon>
  );
}
