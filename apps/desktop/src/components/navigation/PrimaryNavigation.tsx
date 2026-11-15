import React from "react";
import { NavLink } from "react-router-dom";
import type { RuntimeStatus } from "@/types/media";
import styles from "./nav.module.css";

interface NavItem {
  readonly to: string;
  readonly label: string;
  readonly icon: React.ReactElement;
}

function icon(path: React.ReactElement) {
  return (
    <svg
      className={styles.icon}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {path}
    </svg>
  );
}

const ITEMS: readonly NavItem[] = [
  {
    to: "/",
    label: "Home",
    icon: icon(<path d="M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" />),
  },
  {
    to: "/movies",
    label: "Movies",
    icon: icon(
      <>
        <rect x="2.5" y="4.5" width="19" height="15" rx="2.5" />
        <path d="M7 4.5v15M17 4.5v15M2.5 12h19M2.5 8.2h4.5M2.5 15.8h4.5M17 8.2h4.5M17 15.8h4.5" />
      </>,
    ),
  },
  {
    to: "/shows",
    label: "TV Shows",
    icon: icon(
      <>
        <rect x="2.5" y="4.5" width="19" height="15" rx="2.5" />
        <path d="M8 9.5h8M8 13.5h5" />
      </>,
    ),
  },
  {
    to: "/library",
    label: "Library",
    icon: icon(
      <>
        <path d="M4 4.5h5v15H4zM10.5 4.5h4v15h-4zM16.2 5.4l3.6 1.2-4.2 13.2-3.3-1.1z" />
      </>,
    ),
  },
  {
    to: "/watchlist",
    label: "Watchlist",
    icon: icon(
      <path d="M6.5 3.5h11a1 1 0 0 1 1 1v16l-6.5-4-6.5 4v-16a1 1 0 0 1 1-1z" />,
    ),
  },
  {
    to: "/search",
    label: "Search",
    icon: icon(
      <>
        <circle cx="11" cy="11" r="6.5" />
        <path d="m16 16 4.5 4.5" />
      </>,
    ),
  },
  {
    to: "/settings",
    label: "Settings",
    icon: icon(
      <>
        <circle cx="12" cy="12" r="3.2" />
        <path d="M12 2.8v2.6M12 18.6v2.6M21.2 12h-2.6M5.4 12H2.8M18.5 5.5l-1.8 1.8M7.3 16.7l-1.8 1.8M18.5 18.5l-1.8-1.8M7.3 7.3 5.5 5.5" />
      </>,
    ),
  },
];

export interface PrimaryNavigationProps {
  readonly runtimeStatus: RuntimeStatus;
  readonly downloadsEnabled: boolean;
}

export function PrimaryNavigation({
  runtimeStatus,
  downloadsEnabled,
}: PrimaryNavigationProps) {
  const items = downloadsEnabled
    ? ITEMS
    : ITEMS.filter((item) => item.to !== "/downloads");

  const statusLabel =
    runtimeStatus.state === "ready"
      ? "Backend ready"
      : runtimeStatus.state === "starting"
        ? "Backend starting"
        : runtimeStatus.state === "unhealthy"
          ? "Backend unhealthy"
          : runtimeStatus.state === "stopped"
            ? "Backend stopped"
            : runtimeStatus.state === "absent"
              ? "Backend unavailable"
              : "Backend status unknown";

  const statusClass =
    runtimeStatus.state === "ready"
      ? styles.statusReady
      : runtimeStatus.state === "unhealthy" || runtimeStatus.state === "absent"
        ? styles.statusProblem
        : "";

  return (
    <nav className={styles.rail} aria-label="Primary">
      <div className={styles.brand}>
        <span className={styles.brandMark} aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            width="18"
            height="18"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
            focusable="false"
          >
            <path d="M4 7.5 12 3l8 4.5v9L12 21l-8-4.5z" />
            <path d="M12 12v9M12 12 4 7.5M12 12l8-4.5" />
          </svg>
        </span>
        <span className={styles.brandText}>MEDIA_HAVEN</span>
      </div>

      <ul className={styles.list}>
        {items.map((item) => (
          <li key={item.to}>
            <NavLink
              to={item.to}
              end={item.to === "/"}
              className={({ isActive }) =>
                `${styles.item} ${isActive ? styles.itemActive : ""}`
              }
            >
              {item.icon}
              <span className={styles.label}>{item.label}</span>
            </NavLink>
          </li>
        ))}
      </ul>

      <div className={styles.footer}>
        <div className={styles.status}>
          <span
            className={`${styles.statusDot} ${statusClass}`}
            aria-hidden="true"
          />
          <span>{statusLabel}</span>
        </div>
      </div>
    </nav>
  );
}
