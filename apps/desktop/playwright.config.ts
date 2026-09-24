import { defineConfig, devices } from '@playwright/test';

/**
 * E2E do front (F08-08): o app de verdade no Chromium, com o core em Rust trocado pelo
 * core falso de `src/e2e/fakeCore.ts` (`vite --mode e2e`). Ver docs/fases/FASE-08.
 */
const PORT = 5181;

export default defineConfig({
  testDir: 'e2e',
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  // Sem novas tentativas: teste instável é bug (R10), não algo a esconder.
  retries: 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: 'retain-on-failure',
    viewport: { width: 1280, height: 800 },
    launchOptions: process.env.PW_CHROMIUM_PATH
      ? { executablePath: process.env.PW_CHROMIUM_PATH }
      : {},
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } },
    },
  ],
  webServer: {
    command: `pnpm exec vite --mode e2e --port ${PORT} --strictPort`,
    port: PORT,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
