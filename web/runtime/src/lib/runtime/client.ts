import { derived, writable } from 'svelte/store';
import type {
	RegisteredApp,
	RuntimeEvent,
	RuntimeState,
	SocketMessage
} from '@ghost/runtime-contracts';

const mockState: RuntimeState = {
	sessionId: 'preview-8f32',
	phase: 'ready',
	startedAtUnixMs: Date.now() - 188_000,
	updatedAtUnixMs: Date.now(),
	fl: {
		pid: 18320,
		launchedByGhost: true,
		windowReady: true,
		debugPort: 9222,
		gopherReady: true,
		gopherTarget: 'gopher',
		gopherToolCount: 34
	},
	app: {
		profile: 'workspace',
		pid: 19444,
		endpoint: 'http://127.0.0.1:48775',
		healthy: true,
		scriptingConnected: true,
		threadId: 'thread-preview',
		lastError: null
	},
	lastError: null
};

const mockEvents: RuntimeEvent[] = [
	{
		sequence: 18,
		timestampUnixMs: Date.now() - 8_000,
		component: 'workspace',
		event: 'health_ready',
		severity: 'info',
		data: { endpoint: mockState.app.endpoint }
	},
	{
		sequence: 17,
		timestampUnixMs: Date.now() - 13_000,
		component: 'scripting',
		event: 'connected',
		severity: 'info',
		data: {}
	},
	{
		sequence: 16,
		timestampUnixMs: Date.now() - 21_000,
		component: 'gopher',
		event: 'target_discovered',
		severity: 'info',
		data: { tools: 34 }
	},
	{
		sequence: 15,
		timestampUnixMs: Date.now() - 31_000,
		component: 'fl',
		event: 'window_ready',
		severity: 'info',
		data: { pid: 18320 }
	}
];

const mockApps: RegisteredApp[] = [
	{
		id: 'workspace',
		displayName: 'FL Workspace',
		route: '/apps/workspace',
		uiUrl: mockState.app.endpoint,
		endpoint: mockState.app.endpoint,
		healthy: true
	}
];
const initialState: RuntimeState = {
	...mockState,
	sessionId: 'pending',
	phase: 'booting',
	fl: {
		...mockState.fl,
		pid: null,
		launchedByGhost: false,
		windowReady: false,
		gopherReady: false,
		gopherTarget: null,
		gopherToolCount: null
	},
	app: { ...mockState.app, pid: null, healthy: false, scriptingConnected: null, threadId: null }
};

export const runtimeState = writable<RuntimeState>(initialState);
export const runtimeEvents = writable<RuntimeEvent[]>([]);
const registeredAppDescriptors = writable<RegisteredApp[]>([]);
export function mergeAppHealth(apps: RegisteredApp[], state: RuntimeState): RegisteredApp[] {
	return apps.map((app) =>
		app.id === state.app.profile ? { ...app, healthy: state.app.healthy } : app
	);
}
export const registeredApps = derived(
	[registeredAppDescriptors, runtimeState],
	([$registeredAppDescriptors, $runtimeState]) =>
		mergeAppHealth($registeredAppDescriptors, $runtimeState)
);
export const connectionState = writable<'connecting' | 'live' | 'reconnecting' | 'mock'>(
	'connecting'
);
export const commandError = writable<string | null>(null);

let socket: WebSocket | undefined;
let retry = 0;
let timer: ReturnType<typeof setTimeout> | undefined;
let stopped = false;

export function isMockMode(): boolean {
	return (
		typeof location !== 'undefined' && new URLSearchParams(location.search).get('mock') === '1'
	);
}

export async function connectRuntime(): Promise<() => void> {
	stopped = false;
	if (isMockMode()) {
		runtimeState.set(mockState);
		runtimeEvents.set(mockEvents);
		registeredAppDescriptors.set(mockApps);
		connectionState.set('mock');
		return disconnectRuntime;
	}
	connectSocket();
	void refreshSnapshot().catch(() => undefined);
	document.addEventListener('visibilitychange', onVisibility);
	return disconnectRuntime;
}

function connectSocket(): void {
	if (stopped || document.hidden) return;
	connectionState.set(retry ? 'reconnecting' : 'connecting');
	const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
	socket = new WebSocket(`${scheme}//${location.host}/api/ws`);
	socket.addEventListener('open', () => {
		retry = 0;
		connectionState.set('live');
	});
	socket.addEventListener('message', ({ data }) =>
		applySocketMessage(JSON.parse(String(data)) as SocketMessage)
	);
	socket.addEventListener('close', scheduleReconnect);
	socket.addEventListener('error', () => socket?.close());
}

function applySocketMessage(message: SocketMessage): void {
	runtimeState.set(message.state);
	if (message.kind === 'snapshot') runtimeEvents.set(message.events.toReversed());
	else runtimeEvents.update((events) => [message.event, ...events].slice(0, 200));
}

function scheduleReconnect(): void {
	if (stopped || document.hidden) return;
	connectionState.set('reconnecting');
	const delay = Math.min(10_000, 500 * 2 ** Math.min(retry++, 5));
	timer = setTimeout(connectSocket, delay);
}

function onVisibility(): void {
	if (document.hidden) {
		socket?.close();
		if (timer) clearTimeout(timer);
	} else {
		void refreshSnapshot();
		connectSocket();
	}
}

async function refreshSnapshot(): Promise<void> {
	const [stateResponse, eventsResponse, appsResponse] = await Promise.all([
		fetch('/api/state'),
		fetch('/api/events'),
		fetch('/api/apps')
	]);
	if (!stateResponse.ok) throw new Error(`runtime state returned ${stateResponse.status}`);
	runtimeState.set((await stateResponse.json()) as RuntimeState);
	if (eventsResponse.ok)
		runtimeEvents.set(((await eventsResponse.json()) as RuntimeEvent[]).toReversed());
	if (appsResponse.ok) registeredAppDescriptors.set((await appsResponse.json()) as RegisteredApp[]);
}

export async function runCommand(path: string): Promise<void> {
	commandError.set(null);
	if (isMockMode()) {
		await new Promise((resolve) => setTimeout(resolve, 180));
		return;
	}
	const response = await fetch(path, { method: 'POST' });
	if (!response.ok) {
		const message = await response.text();
		commandError.set(message);
		throw new Error(message);
	}
}

export function disconnectRuntime(): void {
	stopped = true;
	socket?.close();
	if (timer) clearTimeout(timer);
	document.removeEventListener('visibilitychange', onVisibility);
}

export function elapsed(start: number, now = Date.now()): string {
	const seconds = Math.max(0, Math.floor((now - start) / 1000));
	if (seconds < 60) return `${seconds}s`;
	const minutes = Math.floor(seconds / 60);
	return minutes < 60
		? `${minutes}m ${seconds % 60}s`
		: `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}
