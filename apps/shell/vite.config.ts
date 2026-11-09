import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

const CORE_DEV = process.env.NEXUS_DEV_HTTP || 'http://127.0.0.1:8799';

export default defineConfig({
  plugins: [solid()],
  base: './',
  server: {
    host: '127.0.0.1',
    port: 5173,
    proxy: {
      '/rpc': { target: CORE_DEV.replace('http', 'ws'), ws: true },
      '/art': { target: CORE_DEV },
    },
  },
  build: { target: 'chrome120', outDir: 'dist', assetsInlineLimit: 0, sourcemap: false },
});
