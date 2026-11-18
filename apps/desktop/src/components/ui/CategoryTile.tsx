import { Link } from "react-router-dom";

import { Icon, type IconName } from "./Icon";
import styles from "./category.module.css";

export function CategoryTile({
  to,
  label,
  count,
  icon,
}: {
  to: string;
  label: string;
  count: string;
  icon: IconName;
}) {
  return (
    <Link to={to} className={styles.tile} data-focusable="true">
      <span className={styles.glyph}>
        <Icon name={icon} />
      </span>
      <span className={styles.copy}>
        <span className={styles.label}>{label}</span>
        <span className={styles.count}>{count}</span>
      </span>
      <Icon name="chevronRight" className={styles.chevron} />
    </Link>
  );
}
