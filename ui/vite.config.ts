import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// La app Tauri carica la build da `dist/`; `npm run dev` serve la stessa UI
// nel browser con un backend simulato (vedi src/lib/api.ts).
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: 'es2022', outDir: 'dist', emptyOutDir: true },
});
