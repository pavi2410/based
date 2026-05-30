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
