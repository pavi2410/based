// @ts-check
import { defineConfig } from "astro/config";

/** Public origin for canonical, Open Graph, and sitemap URLs (override with SITE for previews). */
const PRODUCTION_SITE = "https://based.pavi2410.com";

// https://astro.build/config
export default defineConfig({
  site: process.env.SITE ?? PRODUCTION_SITE,
  devToolbar: {
    enabled: false,
  },
});
