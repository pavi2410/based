# Based landing page

Static marketing site for [Based](https://github.com/pavi2410/based), built with Astro and deployed to Cloudflare Workers static assets.

## Prerequisites

- Node.js >= 22.12
- [pnpm](https://pnpm.io/)
- [Wrangler CLI](https://developers.cloudflare.com/workers/wrangler/) (installed as a dev dependency)

## Development

```bash
# From repo root
mise run web-dev

# Or directly
cd apps/web && pnpm install && pnpm dev
```

Open [http://localhost:4321](http://localhost:4321).

## Build & preview

```bash
mise run web-build          # outputs to apps/web/dist/
mise run web-preview        # build + wrangler dev (local Workers static assets)
```

## Deploy (Cloudflare Workers Builds)

Connect this repo in the Cloudflare dashboard and let Workers Builds handle CI/CD — no GitHub Actions required.

### One-time setup

1. Cloudflare dashboard → **Compute → Workers & Pages → Create application**
2. Choose **Import a repository** and connect GitHub
3. Select the `based` repo
4. Configure the build:

| Setting | Value |
|---------|-------|
| Root directory | `apps/web` |
| Build command | `pnpm install && SITE=https://your-domain.com pnpm build` |
| Deploy command | `pnpm exec wrangler deploy` |

Set **`SITE`** to your public origin (custom domain or `*.workers.dev`) so canonical URLs and Open Graph/Twitter image links are absolute. Add the same variable under **Settings → Variables** in the Worker build configuration.

5. Save and deploy — Cloudflare builds on every push to the connected branch
6. Optionally attach a custom domain under the Worker settings

### Manual deploy (optional)

For one-off deploys from your machine:

```bash
cd apps/web && pnpm exec wrangler login
mise run web-deploy
```

## Stack notes

- **Static Astro** — no `@astrojs/cloudflare` adapter (not needed for static output)
- **Wrangler** — uploads `dist/` as Worker static assets via `wrangler.jsonc`
- **Star count** — fetched from the GitHub API at build time

## Project structure

```text
apps/web/
├── src/
│   ├── components/   # Hero, GitHubStar, Screenshot, SignalChips, etc.
│   ├── layouts/
│   ├── lib/github.ts
│   ├── pages/index.astro
│   └── styles/
├── public/           # favicon, og.png (1200×630), engine icons, hero screenshot
└── wrangler.jsonc
```
