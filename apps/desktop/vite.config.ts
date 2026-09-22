import { fileURLToPath, URL } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  // Porta fixa: o Tauri aponta para ela em tauri.conf.json (devUrl).
  server: { port: 5173, strictPort: true },
  // O Tauri empacota o dist; sourcemap só em dev.
  build: { target: 'es2022', sourcemap: false },
  test: { environment: 'jsdom', globals: true },
});
