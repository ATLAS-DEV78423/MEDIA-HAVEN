import { describe, expect, it } from "vitest";

import { artFor } from "./catalog";

describe("catalog artwork fallback", () => {
  it("uses a neutral placeholder instead of an unrelated film still", () => {
    const art = artFor("catalog-series-123", "The Mentalist");
    expect(art.hero).toBe("/artwork/placeholder.svg");
    expect(art.still).toBe("/artwork/placeholder.svg");
  });
});
