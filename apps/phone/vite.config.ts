import { defineConfig } from 'vite';

export default defineConfig({
  base: '/phone/',
  server: { port: 5174, fs: { allow: ['../..'] } },
  build: { outDir: 'dist', chunkSizeWarningLimit: 1000 },
});
