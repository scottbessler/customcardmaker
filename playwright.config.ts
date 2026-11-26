import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { defineConfig, devices } from "@playwright/test";
import { IPHONE_PORTRAIT } from "./tests/e2e/devices";
import type { DeviceOptions } from "./tests/e2e/fixtures";

/**
/**
 * A database of its own for every run. The default `data/` folder is shared by
 * every branch checked out here, and a run on a branch with a newer migration
 * leaves it ahead of an older build, which then (rightly) refuses to open a
 * database from the future. Set E2E_DATA_PATH to keep one between runs.
 */
const DATA_PATH = process.env.E2E_DATA_PATH ?? mkdtempSync(join(tmpdir(), "ccm-e2e-"));

/** In CI the server is built by its own job; locally cargo still builds it. */
// The app looks for Creative Commons art at a stand-in (tests/e2e/fake-openverse.mjs),
// so no test depends on, or spends the quota of, the real service.
const OPENVERSE = "http://127.0.0.1:18081";
const serverCommand = process.env.E2E_SERVER_BIN
  ? `PASSKEY_DISABLED=1 DATA_PATH="${DATA_PATH}" OPENVERSE_URL=${OPENVERSE} PORT=18080 ${process.env.E2E_SERVER_BIN}`
  : `PASSKEY_DISABLED=1 DATA_PATH="${DATA_PATH}" OPENVERSE_URL=${OPENVERSE} PORT=18080 cargo run`;

export default defineConfig<DeviceOptions>({
  testDir: "./tests/e2e",
  snapshotPathTemplate: "{testDir}/{testFileName}-snapshots/{arg}-{projectName}{ext}",
  expect: {
    toHaveScreenshot: {
      animations: "disabled",
      maxDiffPixelRatio: 0.01,
    },
  },
  reporter: [["html", { outputFolder: "playwright-report", open: "never" }]],
  use: {
    baseURL: process.env.TEST_BASE_URL || "http://127.0.0.1:18080",
    trace: "on-first-retry",
    emulatedDevice: null,
    launchOptions: {
      // GitHub's job containers give /dev/shm the default 64MB; a full-page
      // screenshot of a long page outgrows it and Chromium dies. Not a rendering
      // flag — it only moves the backing store to /tmp.
      args: [
        "--disable-dev-shm-usage",
        // Rasterise text the same way on every host: no hinting, no subpixel
        // positioning, no LCD filter, one colour profile. With the pinned fonts
        // in rendering.ts this is what makes a local Linux run match CI.
        "--font-render-hinting=none",
        "--disable-font-subpixel-positioning",
        "--disable-lcd-text",
        "--force-color-profile=srgb",
      ],
    },
  },
  ...(process.env.TEST_BASE_URL
    ? {}
    : {
        webServer: [
          {
            command: "node tests/e2e/fake-openverse.mjs",
            url: `${OPENVERSE}/health`,
            reuseExistingServer: true,
            timeout: 30_000,
          },
          {
            // Passkeys cannot be driven headlessly; the server signs in by username.
            command: serverCommand,
            url: "http://127.0.0.1:18080/healthcheck",
            reuseExistingServer: true,
            timeout: 120_000,
          },
        ],
      }),
  projects: [
    {
      name: "chromium-desktop",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 1400 } },
    },
    {
      // Mirrors the phone the app is used on: an iPhone 15/16 Pro running the
      // installed PWA. Chromium still renders it, but the geometry — viewport and
      // safe-area insets both — is the iPhone's, so snapshots match the device.
      name: "chromium-mobile",
      use: {
        browserName: "chromium",
        viewport: IPHONE_PORTRAIT.viewport,
        emulatedDevice: IPHONE_PORTRAIT,
        deviceScaleFactor: 3,
        isMobile: true,
        hasTouch: true,
      },
    },
  ],
});
