import type { CSSProperties, ReactNode } from "react";

export type IconName = "phone" | "bell" | "code" | "settings" | "message" | "plus" | "chevron";

const paths: Record<IconName, ReactNode> = {
  phone: <><rect x="6" y="2.5" width="12" height="19" rx="2.5" /><path d="M10 5h4M11 18.5h2" /></>,
  bell: <><path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9ZM10 21h4" /></>,
  code: <><rect x="3" y="5" width="18" height="14" rx="2" /><path d="m8 10-2 2 2 2m8-4 2 2-2 2m-3-4-2 4" /></>,
  settings: <><path d="M4 6h16M4 12h16M4 18h16" /><path d="M8 4v4m8 2v4m-6 2v4" /></>,
  message: <path d="M5 3h14a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2h-8l-6 3v-3a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Zm2 5h10M7 12h7" />,
  plus: <path d="M12 5v14M5 12h14" />,
  chevron: <path d="m9 5 7 7-7 7" />,
};

export function Icon({ name, size = 18, style }: { name: IconName; size?: number; style?: CSSProperties }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" style={style}>{paths[name]}</svg>;
}
