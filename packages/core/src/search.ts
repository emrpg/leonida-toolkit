import type { WorldPoint } from './coords.js';
import { distance } from './coords.js';
import type { IrlStatus, Landmark } from './landmarks.js';

export interface LandmarkFilter {
  text?: string;
  tags?: readonly string[];
  statuses?: readonly IrlStatus[];
  districts?: readonly string[];
  positionedOnly?: boolean;
}

function fold(s: string): string {
  return s.normalize('NFKD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
}

export function filterLandmarks(list: readonly Landmark[], f: LandmarkFilter): Landmark[] {
  const terms = f.text ? fold(f.text).split(/\s+/).filter(Boolean) : [];
  return list.filter((l) => {
    if (f.positionedOnly && !l.ig) return false;
    if (f.tags?.length && !l.tags.some((t) => f.tags!.includes(t))) return false;
    if (f.statuses?.length && !f.statuses.includes(l.irlStatus)) return false;
    if (f.districts?.length && !f.districts.includes(l.district ?? '')) return false;
    if (terms.length) {
      const hay = fold(`${l.id}\n${l.igAddress}\n${l.irlAddress}\n${l.tags.join(' ')}`);
      if (!terms.every((t) => hay.includes(t))) return false;
    }
    return true;
  });
}

export function nearestLandmarks(
  list: readonly Landmark[],
  p: WorldPoint,
  n = 5,
): Array<{ landmark: Landmark; distance: number }> {
  return list
    .filter((l): l is Landmark & { ig: WorldPoint } => l.ig !== null)
    .map((l) => ({ landmark: l as Landmark, distance: distance(l.ig, p) }))
    .sort((a, b) => a.distance - b.distance)
    .slice(0, n);
}

/** GeoJSON (non-geographic CRS: coordinates are Leonida world metres). */
export function toGeoJSON(list: readonly Landmark[]): object {
  return {
    type: 'FeatureCollection',
    crs: { type: 'name', properties: { name: 'urn:leonida:world-metres' } },
    features: list
      .filter((l) => l.ig)
      .map((l) => ({
        type: 'Feature',
        id: l.id,
        geometry: { type: 'Point', coordinates: [l.ig!.x, l.ig!.y] },
        properties: {
          name: l.name,
          igAddress: l.igAddress,
          district: l.district,
          irlAddress: l.irlAddress,
          irlStatus: l.irlStatus,
          irl: l.irl,
          tags: l.tags,
          color: `#${l.color}`,
          source: l.source,
        },
      })),
  };
}
