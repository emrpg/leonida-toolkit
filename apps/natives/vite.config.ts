import { defineConfig } from 'vite';

export default defineConfig({
  base: '/natives/',
  server: { port: 5175, fs: { allow: ['../..'] } },
  build: { outDir: 'dist', chunkSizeWarningLimit: 1000 },
});
