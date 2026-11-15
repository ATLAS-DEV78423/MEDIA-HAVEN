/**
 * Deterministic cover art for the development fixtures.
 *
 * The mock must render a complete library offline, with no network fetch
 * and no bundled binary assets. Each title gets a stable abstract poster
 * derived from a hash of its name, so screenshots and tests are
 * reproducible.
 *
 * This is fixture art only. In the product, artwork comes from the local
 * cache (ADR-0006) and a missing variant is a designed placeholder.
 */

import type { ArtworkRef, ArtworkVariant } from "@/types/media";

function hashOf(input: string): number {
  let hash = 2166136261;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function hueFor(seed: number): number {
  // Spread hues across the wheel, but skip the violet band so the library
  // does not read as a single purple wash.
  const raw = seed % 320;
  return raw > 250 ? raw - 60 : raw;
}

function initialsOf(title: string): string {
  const words = title.replace(/[^A-Za-z0-9 ]/g, " ").split(/\s+/).filter(Boolean);
  if (words.length === 0) return "•";
  if (words.length === 1) return words[0]!.slice(0, 2).toUpperCase();
  return (words[0]![0]! + words[1]![0]!).toUpperCase();
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

const VARIANTS: Record<ArtworkVariant, { w: number; h: number }> = {
  poster: { w: 600, h: 900 },
  backdrop: { w: 1600, h: 900 },
  still: { w: 960, h: 540 },
};

export function createArtwork(
  title: string,
  variant: ArtworkVariant,
  subtitle?: string,
): ArtworkRef {
  const seed = hashOf(`${title}:${variant}`);
  const hue = hueFor(seed);
  const { w, h } = VARIANTS[variant];
  const cx = 20 + (seed % 60);
  const cy = 18 + ((seed >> 3) % 44);
  const initials = initialsOf(title);
  const caption = escapeXml(subtitle ?? title);

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
<defs>
<linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
<stop offset="0" stop-color="hsl(${hue} 42% 17%)"/>
<stop offset="0.55" stop-color="hsl(${(hue + 26) % 360} 34% 11%)"/>
<stop offset="1" stop-color="hsl(${(hue + 52) % 360} 28% 6%)"/>
</linearGradient>
<radialGradient id="l" cx="${cx}%" cy="${cy}%" r="52%">
<stop offset="0" stop-color="hsl(${(hue + 16) % 360} 70% 62%)" stop-opacity="0.55"/>
<stop offset="0.6" stop-color="hsl(${(hue + 16) % 360} 60% 40%)" stop-opacity="0.12"/>
<stop offset="1" stop-color="hsl(${(hue + 16) % 360} 60% 20%)" stop-opacity="0"/>
</radialGradient>
<linearGradient id="v" x1="0" y1="0" x2="0" y2="1">
<stop offset="0" stop-color="#000" stop-opacity="0"/>
<stop offset="0.62" stop-color="#000" stop-opacity="0"/>
<stop offset="1" stop-color="#000" stop-opacity="0.55"/>
</linearGradient>
</defs>
<rect width="${w}" height="${h}" fill="url(#g)"/>
<rect width="${w}" height="${h}" fill="url(#l)"/>
<g opacity="0.13" fill="none" stroke="#fff" stroke-width="1.5">
<circle cx="${(cx / 100) * w}" cy="${(cy / 100) * h}" r="${h * 0.24}"/>
<circle cx="${(cx / 100) * w}" cy="${(cy / 100) * h}" r="${h * 0.36}"/>
</g>
<text x="${w * 0.5}" y="${h * 0.52}" text-anchor="middle" fill="#fff" fill-opacity="0.2"
 font-family="Segoe UI, Helvetica, Arial, sans-serif" font-weight="700" font-size="${h * 0.2}"
 letter-spacing="-4">${escapeXml(initials)}</text>
<rect width="${w}" height="${h}" fill="url(#v)"/>
<text x="${w * 0.5}" y="${h * 0.93}" text-anchor="middle" fill="#fff" fill-opacity="0.78"
 font-family="Segoe UI, Helvetica, Arial, sans-serif" font-weight="600" font-size="${h * 0.042}"
 letter-spacing="1">${caption}</text>
</svg>`;

  return {
    variant,
    url: `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`,
    alt: `${title} cover art`,
  };
}
