const GITHUB_API = "https://api.github.com/repos/pavi2410/based";

/** -1 sentinel: API responded but we are rate-limited. Distinct from null (network/other error). */
export const RATE_LIMITED = -1 as const;

function isRateLimited(res: Response): boolean {
  return res.status === 429 ||
    (res.status === 403 && res.headers.get("X-RateLimit-Remaining") === "0");
}

export const REPO_URL = "https://github.com/pavi2410/based";
export const RELEASES_URL = "https://github.com/pavi2410/based/releases";
export const DISCUSSIONS_URL = "https://github.com/pavi2410/based/discussions";

export async function fetchStarCount(): Promise<number | null> {
  try {
    const response = await fetch(GITHUB_API, {
      headers: {
        Accept: "application/vnd.github+json",
        "User-Agent": "based-landing",
      },
    });

    if (isRateLimited(response)) return RATE_LIMITED;
    if (!response.ok) return null;

    const data = (await response.json()) as { stargazers_count?: number };
    return data.stargazers_count ?? null;
  } catch {
    return null;
  }
}

export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
  download_count: number;
  /** GitHub-computed SHA-256, e.g. `sha256:abc…` */
  digest?: string;
}

export interface LatestRelease {
  tag_name: string;
  assets: ReleaseAsset[];
  rateLimited?: true;
}

export interface GitHubRelease {
  tag_name: string;
  assets: ReleaseAsset[];
}

export type AllReleasesResult = GitHubRelease[] | "rate_limited" | null;

const githubHeaders = {
  Accept: "application/vnd.github+json",
  "User-Agent": "based-landing",
};

export async function fetchLatestRelease(): Promise<LatestRelease | null> {
  try {
    const response = await fetch(`${GITHUB_API}/releases/latest`, { headers: githubHeaders });

    if (isRateLimited(response)) {
      return { tag_name: "", assets: [], rateLimited: true };
    }
    if (!response.ok) return null;
    return (await response.json()) as LatestRelease;
  } catch {
    return null;
  }
}

/** All releases (paginated). Used for lifetime download stats. */
export async function fetchAllReleases(): Promise<AllReleasesResult> {
  try {
    const response = await fetch(`${GITHUB_API}/releases?per_page=100`, { headers: githubHeaders });
    if (isRateLimited(response)) return "rate_limited";
    if (!response.ok) return null;
    return (await response.json()) as GitHubRelease[];
  } catch {
    return null;
  }
}

/** Sum download_count across every asset in every release. */
export function sumAllDownloads(releases: GitHubRelease[]): number {
  return releases.reduce(
    (total, r) => total + r.assets.reduce((s, a) => s + a.download_count, 0),
    0,
  );
}

/** Lifetime downloads for assets whose filename matches any pattern, across all releases. */
export function lifetimeAssetDownloads(
  releases: AllReleasesResult,
  ...patterns: RegExp[]
): number {
  if (releases === "rate_limited") return RATE_LIMITED;
  if (!releases) return 0;
  return releases.reduce(
    (total, r) =>
      total +
      r.assets
        .filter((a) => patterns.some((p) => p.test(a.name)))
        .reduce((s, a) => s + a.download_count, 0),
    0,
  );
}

/** Find the first asset on the latest release matching any pattern. */
export function findAssetInfo(
  release: LatestRelease | null,
  ...patterns: RegExp[]
): { url: string; digest?: string; name: string } | null {
  if (!release || release.rateLimited) return null;
  for (const pattern of patterns) {
    const asset = release.assets.find((a) => pattern.test(a.name));
    if (asset) {
      return {
        url: asset.browser_download_url,
        digest: asset.digest,
        name: asset.name,
      };
    }
  }
  return null;
}

/** Strip `sha256:` prefix for display and local verification. */
export function digestHex(digest: string | undefined): string | null {
  if (!digest) return null;
  return digest.replace(/^sha256:/i, "");
}

/** Middle-truncated hex for compact UI. */
export function digestShort(hex: string): string {
  if (hex.length <= 20) return hex;
  return `${hex.slice(0, 8)}…${hex.slice(-8)}`;
}

/** Find the first asset whose filename matches any of the given patterns. */
export function findAssetUrl(
  release: LatestRelease | null,
  ...patterns: RegExp[]
): string | null {
  return findAssetInfo(release, ...patterns)?.url ?? null;
}
