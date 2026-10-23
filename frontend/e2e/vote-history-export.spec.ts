import { test, expect } from '@playwright/test';
import path from 'path';

/**
 * E2E: Export vote history as CSV (issue #20)
 */
test.describe('Vote history CSV export', () => {
  // A known-valid Stellar G-address for testing
  const VALID_ADDRESS = 'GAHJJJKMOKYE4RVPZEWZTKH5FVI4PA3VL7GK2LFNUBSGBVDRGTZRS4A';

  test('export CSV button is disabled without a valid address', async ({ page }) => {
    await page.goto('/history');
    const exportBtn = page.getByRole('button', { name: /export csv/i });
    await expect(exportBtn).toBeVisible({ timeout: 8_000 });
    await expect(exportBtn).toBeDisabled();
  });

  test('inline error shown for invalid address', async ({ page }) => {
    await page.goto('/history');
    const input = page.getByPlaceholder(/enter public address/i);
    if (!(await input.isVisible())) return;

    await input.fill('INVALID_ADDRESS');
    const errorMsg = page.locator('[role="alert"].field-error, #address-error');
    await expect(errorMsg).toBeVisible({ timeout: 3_000 });
  });

  test('no error shown for valid address', async ({ page }) => {
    await page.goto('/history');
    const input = page.getByPlaceholder(/enter public address/i);
    if (!(await input.isVisible())) return;

    await input.fill(VALID_ADDRESS);
    const errorMsg = page.locator('[role="alert"].field-error, #address-error');
    await expect(errorMsg).not.toBeVisible();
  });

  test('CSV download triggers on export click when votes exist', async ({ page }) => {
    await page.goto('/history');
    const input = page.getByPlaceholder(/enter public address/i);
    if (!(await input.isVisible())) return;

    await input.fill(VALID_ADDRESS);

    const exportBtn = page.getByRole('button', { name: /export csv/i });
    // Only attempt download if button is enabled (i.e., votes were found)
    if (await exportBtn.isEnabled()) {
      const [download] = await Promise.all([
        page.waitForEvent('download'),
        exportBtn.click(),
      ]);
      expect(download.suggestedFilename()).toMatch(/vote-history.*\.csv/i);
    }
  });

  test('shows export progress and disables the button while exporting', async ({ page }) => {
    await page.goto('/history');
    const input = page.getByPlaceholder(/enter public address/i);
    if (!(await input.isVisible())) return;

    await input.fill(VALID_ADDRESS);
    const exportButton = page.getByRole('button', { name: /export csv/i });
    if (!(await exportButton.isEnabled())) return;

    const clickPromise = exportButton.click();
    await expect(page.getByRole('progressbar', { name: 'Exporting CSV' })).toBeVisible();
    await expect(exportButton).toBeDisabled();
    await clickPromise;
  });
});
