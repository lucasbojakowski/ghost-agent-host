import { describe, expect, it } from 'vitest';
import type { RegisteredApp, RuntimeState } from '@ghost/runtime-contracts';
import { elapsed, mergeAppHealth } from './client';

describe('elapsed', () => {
	it('formats runtime durations compactly', () => {
		expect(elapsed(0, 3_725_000)).toBe('1h 2m');
	});

	it('projects live runtime health onto the registered app descriptor', () => {
		const state = {
			app: { profile: 'workspace', healthy: true }
		} as RuntimeState;
		const apps = [{ id: 'workspace', healthy: false }] as RegisteredApp[];

		expect(mergeAppHealth(apps, state)[0]?.healthy).toBe(true);
	});
});
