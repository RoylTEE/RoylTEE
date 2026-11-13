import { defineConfig } from 'astro/config';
import vercel from '@astrojs/vercel';
// assetsInlineLimit 0: never inline a script into the HTML. The strict CSP (vercel.json) allows script-src 'self' only.
export default defineConfig({ output: 'static', adapter: vercel(), vite: { build: { assetsInlineLimit: 0 } } });
