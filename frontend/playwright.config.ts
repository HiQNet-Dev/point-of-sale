import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { defineConfig, devices } from '@playwright/test'

const baseURL = process.env.E2E_FRONTEND_URL ?? 'http://127.0.0.1:5173'
const artifactsDir = process.env.PLAYWRIGHT_ARTIFACT_DIR ?? join(tmpdir(), 'hiqnet-pos-playwright')

export default defineConfig({
  testDir: './e2e',
  globalSetup: './e2e/global-setup.ts',
  globalTeardown: './e2e/global-teardown.ts',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [['list'], ['html', { outputFolder: join(artifactsDir, 'report'), open: 'never' }]],
  outputDir: join(artifactsDir, 'results'),
  use: {
    ...devices['Desktop Chrome'],
    baseURL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
})
