/**
 * Visual regression tests for key VoteChain UI states — Issue #300
 *
 * Each test navigates to a route, waits for the network to settle, then
 * compares a screenshot against a stored baseline.  The maxDiffPixelRatio
 * of 0.001 (0.1 %) is configured globally in playwright.config.ts and also
 * applied per-call here for explicitness.
 *
 * Baseline PNGs are generated on the first run and stored in the directory
 * configured by `snapshotDir` in playwright.config.ts
 * (`frontend/e2e/snapshots/`).  Re-generate baselines by running:
 *
 *   npx playwright test e2e/visual-regression.spec.ts --update-snapshots
 */

import { test, expect } from '@playwright/test';

test.describe('Visual regression — key UI states', () => {
  test('ProposalList matches snapshot', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await expect(page).toHaveScreenshot('proposal-list.png', { maxDiffPixelRatio: 0.001 });
  });

  test('VotingPanel matches snapshot', async ({ page }) => {
    await page.goto('/vote/P-101');
    await page.waitForLoadState('networkidle');
    await expect(page).toHaveScreenshot('voting-panel.png', { maxDiffPixelRatio: 0.001 });
  });

  test('VoteHistory matches snapshot', async ({ page }) => {
    await page.goto('/history');
    await page.waitForLoadState('networkidle');
    await expect(page).toHaveScreenshot('vote-history.png', { maxDiffPixelRatio: 0.001 });
  });

  test('ProposalDetail matches snapshot', async ({ page }) => {
    await page.goto('/proposal/P-101');
    await page.waitForLoadState('networkidle');
    await expect(page).toHaveScreenshot('proposal-detail.png', { maxDiffPixelRatio: 0.001 });
  });
});
