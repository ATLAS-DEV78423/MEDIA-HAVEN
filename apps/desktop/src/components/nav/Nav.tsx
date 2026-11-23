import { Link, NavLink } from "react-router-dom";

import { Icon } from "@/components/ui/Icon";
import { useAppState } from "@/state/AppState";
import styles from "./nav.module.css";

const TABS = [
  { to: "/", label: "Home", end: true },
  { to: "/movies", label: "Movies" },
  { to: "/shows", label: "TV Shows" },
  { to: "/anime", label: "Anime" },
  { to: "/library", label: "Library" },
  { to: "/watchlist", label: "My List" },
  { to: "/queue", label: "Queue" },
];

export function Nav() {
  const { activeProfile } = useAppState();

  return (
    <>
      <div className={styles.wash} aria-hidden="true" />
      <header className={styles.header}>
        <Link
          to="/"
          className={styles.brand}
          aria-label="MEDIA HAVEN home"
          data-focusable="true"
        >
          <span className={styles.brandMark}>MEDIA HAVEN</span>
        </Link>

        <nav className={styles.pill} aria-label="Primary">
          {TABS.map((tab) => (
            <NavLink
              key={tab.to}
              to={tab.to}
              end={Boolean(tab.end)}
              className={({ isActive }) =>
                `${styles.link} ${isActive ? styles.linkActive : ""}`
              }
              data-focusable="true"
            >
              {tab.label}
            </NavLink>
          ))}
        </nav>

        <div className={styles.actions}>
          <Link
            to="/search"
            className={styles.action}
            aria-label="Search the library"
            data-focusable="true"
          >
            <Icon name="search" />
          </Link>
          <Link
            to="/profiles"
            className={`${styles.avatar} ${activeProfile?.color ? "" : styles.tone1}`}
            style={activeProfile?.color ? { background: activeProfile.color } : undefined}
            aria-label={`Switch profile. Currently ${activeProfile?.name ?? "no profile"}`}
            data-focusable="true"
          >
            {activeProfile?.initial ?? "?"}
          </Link>
        </div>
      </header>
    </>
  );
}
