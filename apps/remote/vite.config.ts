import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  base: './',
  build: { target: 'es2020', outDir: 'dist', assetsInlineLimit: 0 },
  server: { host: '127.0.0.1', port: 5174 },
});
