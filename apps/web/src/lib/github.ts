const GITHUB_API = "https://api.github.com/repos/pavi2410/based";

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

    if (!response.ok) {
      return null;
    }

    const data = (await response.json()) as { stargazers_count?: number };
    return data.stargazers_count ?? null;
  } catch {
    return null;
  }
}

export interface ReleaseAsset {
  name: string;
  browser_download_url: string;
}

export interface LatestRelease {
  tag_name: string;
  assets: ReleaseAsset[];
}

export async function fetchLatestRelease(): Promise<LatestRelease | null> {
  try {
    const response = await fetch(`${GITHUB_API}/releases/latest`, {
      headers: {
        Accept: "application/vnd.github+json",
        "User-Agent": "based-landing",
      },
    });

    if (!response.ok) return null;
    return (await response.json()) as LatestRelease;
  } catch {
    return null;
  }
}

/** Find the first asset whose filename matches any of the given patterns. */
export function findAssetUrl(
  release: LatestRelease | null,
  ...patterns: RegExp[]
): string | null {
  if (!release) return null;
  for (const pattern of patterns) {
    const asset = release.assets.find((a) => pattern.test(a.name));
    if (asset) return asset.browser_download_url;
  }
  return null;
}
