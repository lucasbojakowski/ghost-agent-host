import { expect, test } from '@playwright/test';

test('renders the Signal Console from mock runtime data', async ({ page }) => {
	await page.goto('/?mock=1');
	await expect(page.getByRole('heading', { name: 'Signal Console' })).toBeVisible();
	await expect(page.getByText('Active bootstrap')).toBeVisible();
	await expect(page.getByText('FL Workspace')).toBeVisible();
	await expect(page.getByText('mock', { exact: true })).toBeVisible();
	const shell = await page.evaluate(() => ({
		clientHeight: document.documentElement.clientHeight,
		scrollHeight: document.documentElement.scrollHeight
	}));
	expect(shell.scrollHeight).toBe(shell.clientHeight);
});

test('keeps the runtime navigation usable on a narrow viewport', async ({ page }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.goto('/?mock=1');
	await expect(page.getByRole('navigation', { name: 'Primary navigation' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Signal Console' })).toBeVisible();
});

test('gives app scrolling to the iframe without overflowing the shell', async ({ page }) => {
	await page.route('http://127.0.0.1:48775/**', async (route) => {
		await route.fulfill({
			contentType: 'text/html',
			body: '<!doctype html><html><body style="height: 2400px; margin: 0">Tall app</body></html>'
		});
	});
	await page.goto('/apps/workspace?mock=1');
	await expect(page.getByTitle('FL Workspace application')).toBeVisible();

	const shell = await page.evaluate(() => ({
		clientHeight: document.documentElement.clientHeight,
		scrollHeight: document.documentElement.scrollHeight
	}));
	expect(shell.scrollHeight).toBe(shell.clientHeight);

	const appDocument = await page
		.frameLocator('iframe')
		.locator('html')
		.evaluate((element) => ({
			clientHeight: element.clientHeight,
			scrollHeight: element.scrollHeight
		}));
	expect(appDocument.scrollHeight).toBeGreaterThan(appDocument.clientHeight);
});
