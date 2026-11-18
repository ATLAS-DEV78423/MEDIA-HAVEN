import type { ReactNode } from "react";

import styles from "./Icon.module.css";

export type IconName =
  | "play"
  | "pause"
  | "plus"
  | "check"
  | "search"
  | "chevronLeft"
  | "chevronRight"
  | "chevronDown"
  | "arrowRight"
  | "star"
  | "info"
  | "close"
  | "user"
  | "volume"
  | "mute"
  | "subtitles"
  | "skipBack"
  | "skipForward"
  | "download"
  | "trash"
  | "folder"
  | "clock"
  | "checkCircle"
  | "expand"
  | "home"
  | "film"
  | "tv"
  | "music"
  | "list"
  | "grid"
  | "sliders"
  | "lock";

const GLYPHS: Record<IconName, ReactNode> = {
  play: (
    <path
      d="M8.4 5.6 18.4 12 8.4 18.4V5.6Z"
      fill="currentColor"
      stroke="currentColor"
      strokeLinejoin="round"
    />
  ),
  pause: (
    <>
      <rect x="7" y="5.5" width="3.4" height="13" rx="1.1" fill="currentColor" />
      <rect x="13.6" y="5.5" width="3.4" height="13" rx="1.1" fill="currentColor" />
    </>
  ),
  plus: <path d="M12 5.5v13M5.5 12h13" />,
  check: <path d="m5 12.6 4.4 4.4L19 6.8" />,
  search: (
    <>
      <circle cx="10.8" cy="10.8" r="6.3" />
      <path d="m15.6 15.6 4.2 4.2" />
    </>
  ),
  chevronLeft: <path d="m14.5 5.5-6.5 6.5 6.5 6.5" />,
  chevronRight: <path d="m9.5 5.5 6.5 6.5-6.5 6.5" />,
  chevronDown: <path d="m5.5 9.5 6.5 6.5 6.5-6.5" />,
  arrowRight: <path d="M4.5 12h15m-6-6 6 6-6 6" />,
  star: (
    <path
      d="m12 4.6 2.3 4.9 5.2.7-3.8 3.7.9 5.2-4.6-2.5-4.6 2.5.9-5.2L4.5 10.2l5.2-.7L12 4.6Z"
      fill="currentColor"
      stroke="currentColor"
      strokeLinejoin="round"
    />
  ),
  info: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="M12 11v5.4M12 7.9h.01" />
    </>
  ),
  close: <path d="m6 6 12 12M18 6 6 18" />,
  user: (
    <>
      <circle cx="12" cy="8.4" r="3.6" />
      <path d="M4.8 19.4c1.2-3.1 3.9-4.8 7.2-4.8s6 1.7 7.2 4.8" />
    </>
  ),
  volume: (
    <>
      <path d="M4.5 9.6h3.2L12 6.2v11.6L7.7 14.4H4.5V9.6Z" />
      <path d="M15.4 9.4a3.7 3.7 0 0 1 0 5.2M17.9 7a7.1 7.1 0 0 1 0 10" />
    </>
  ),
  mute: (
    <>
      <path d="M4.5 9.6h3.2L12 6.2v11.6L7.7 14.4H4.5V9.6Z" />
      <path d="m15.4 9.8 4.4 4.4M19.8 9.8l-4.4 4.4" />
    </>
  ),
  subtitles: (
    <>
      <rect x="3.4" y="5.6" width="17.2" height="12.8" rx="2.6" />
      <path d="M7 10.6h4.2M7 14h6.6M14.8 10.6H17M16.2 14H17" />
    </>
  ),
  skipBack: (
    <>
      <path d="M8 6.2v11.6" />
      <path
        d="M18.6 6.6 10 12l8.6 5.4V6.6Z"
        fill="currentColor"
        stroke="currentColor"
        strokeLinejoin="round"
      />
    </>
  ),
  skipForward: (
    <>
      <path d="M16 6.2v11.6" />
      <path
        d="M5.4 6.6 14 12l-8.6 5.4V6.6Z"
        fill="currentColor"
        stroke="currentColor"
        strokeLinejoin="round"
      />
    </>
  ),
  download: <path d="M12 4.8v9.4m0 0 3.6-3.6M12 14.2 8.4 10.6M5 16.4v1.8a1.6 1.6 0 0 0 1.6 1.6h10.8a1.6 1.6 0 0 0 1.6-1.6v-1.8" />,
  trash: (
    <>
      <path d="M4.8 7.2h14.4M9.4 7.2V5.4h5.2v1.8M6.6 7.2l.8 11.2a1.6 1.6 0 0 0 1.6 1.5h6a1.6 1.6 0 0 0 1.6-1.5l.8-11.2" />
      <path d="M10.4 10.8v5.6M13.6 10.8v5.6" />
    </>
  ),
  folder: <path d="M3.8 7.4a1.8 1.8 0 0 1 1.8-1.8h3.6l1.8 2.2h7.4a1.8 1.8 0 0 1 1.8 1.8v8a1.8 1.8 0 0 1-1.8 1.8H5.6a1.8 1.8 0 0 1-1.8-1.8V7.4Z" />,
  clock: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="M12 7.6V12l3.2 2" />
    </>
  ),
  checkCircle: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="m8.2 12.2 2.5 2.5 5-5.2" />
    </>
  ),
  expand: <path d="M9 4.6H4.6V9M15 4.6h4.4V9M9 19.4H4.6V15M15 19.4h4.4V15" />,
  home: <path d="M4.4 10.6 12 4.4l7.6 6.2v7.8a1.6 1.6 0 0 1-1.6 1.6h-3.4v-5.6H9.4V20H6a1.6 1.6 0 0 1-1.6-1.6v-7.8Z" />,
  film: (
    <>
      <rect x="3.6" y="5" width="16.8" height="14" rx="2.2" />
      <path d="M8 5v14M16 5v14M3.6 12h16.8" />
    </>
  ),
  tv: (
    <>
      <rect x="3.4" y="5.2" width="17.2" height="11.4" rx="2.2" />
      <path d="M9 20h6M12 16.6V20" />
    </>
  ),
  music: (
    <>
      <path d="M9.4 17.4V6.6l8.2-1.8v10.8" />
      <circle cx="7" cy="17.8" r="2.4" />
      <circle cx="15.2" cy="15.8" r="2.4" />
    </>
  ),
  list: <path d="M4.6 7h15M4.6 12h15M4.6 17h10" />,
  grid: (
    <>
      <rect x="4" y="4" width="6.6" height="6.6" rx="1.6" />
      <rect x="13.4" y="4" width="6.6" height="6.6" rx="1.6" />
      <rect x="4" y="13.4" width="6.6" height="6.6" rx="1.6" />
      <rect x="13.4" y="13.4" width="6.6" height="6.6" rx="1.6" />
    </>
  ),
  sliders: (
    <>
      <path d="M4.6 7.4h6.2M14.6 7.4h4.8M4.6 16.6h3.2M11.6 16.6h7.8" />
      <circle cx="12.8" cy="7.4" r="1.9" />
      <circle cx="9.8" cy="16.6" r="1.9" />
    </>
  ),
  lock: (
    <>
      <rect x="5.4" y="10.4" width="13.2" height="9.2" rx="2.2" />
      <path d="M8.6 10.4V8.2a3.4 3.4 0 0 1 6.8 0v2.2" />
    </>
  ),
};

export function Icon({
  name,
  className,
  size,
}: {
  name: IconName;
  className?: string | undefined;
  size?: number | undefined;
}) {
  return (
    <svg
      className={className ? `${styles.svg} ${className}` : styles.svg}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...(size ? { width: size, height: size } : {})}
    >
      {GLYPHS[name]}
    </svg>
  );
}
