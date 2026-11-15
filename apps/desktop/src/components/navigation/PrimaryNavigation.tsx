import { Link, NavLink, useNavigate } from "react-router-dom";
import type { RuntimeStatus } from "@/types/media";
import styles from "./nav.module.css";

interface NavItem {
  readonly to: string;
  readonly label: string;
}

const ITEMS: readonly NavItem[] = [
  { to: "/", label: "Home" },
  { to: "/movies", label: "Movies" },
  { to: "/shows", label: "TV Shows" },
  { to: "/library", label: "My Library" },
];

export interface PrimaryNavigationProps {
  readonly runtimeStatus: RuntimeStatus;
  readonly downloadsEnabled: boolean;
}

function SearchIcon() {
  return (
    <svg
      className={styles.searchIcon}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <circle cx="11" cy="11" r="6.5" />
      <path d="m16 16 4.5 4.5" />
    </svg>
  );
}

export function PrimaryNavigation({
  downloadsEnabled,
}: PrimaryNavigationProps) {
  const navigate = useNavigate();

  const items = downloadsEnabled
    ? [...ITEMS, { to: "/downloads", label: "Downloads" }]
    : ITEMS;

  return (
    <header className={styles.rail}>
      <Link to="/" className={styles.brand} aria-label="MEDIA_HAVEN home">
        <span className={styles.brandMark} aria-hidden="true">
          <svg
            viewBox="0 0 24 24"
            width="16"
            height="16"
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
      </Link>

      <nav aria-label="Primary">
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
                {item.label}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>

      <span className={styles.spacer} />

      <label className={styles.search}>
        <SearchIcon />
        <input
          className={styles.searchInput}
          type="search"
          placeholder="Search"
          aria-label="Search your library"
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              const value = (event.target as HTMLInputElement).value.trim();
              if (value) navigate(`/search?q=${encodeURIComponent(value)}`);
            }
          }}
        />
      </label>

      <Link to="/settings" className={styles.profile} aria-label="Profile and settings">
        MH
      </Link>
    </header>
  );
}
