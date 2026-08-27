<script lang="ts">
	import MetricCard from '@ghost/runtime-ui/MetricCard.svelte';
	import StatusBadge from '@ghost/runtime-ui/StatusBadge.svelte';
	import { resolve } from '$app/paths';
	import {
		commandError,
		elapsed,
		registeredApps,
		runCommand,
		runtimeEvents,
		runtimeState
	} from '$lib/runtime/client';
	let pending = $state<string | null>(null);
	async function command(name: string, path: string) {
		pending = name;
		try {
			await runCommand(path);
		} finally {
			pending = null;
		}
	}
</script>

<svelte:head><title>Overview · Ghost Runtime</title></svelte:head>
<section class="hero">
	<div>
		<p class="kicker">System overview</p>
		<h1>Signal Console</h1>
		<p>One surface for the Rust lifecycle, FL Studio, Gopher, and registered Ghost applications.</p>
	</div>
	<div class="hero-actions">
		<button
			class="secondary"
			onclick={() => command('retry', '/api/fl/retry')}
			disabled={pending !== null}>{pending === 'retry' ? 'Retrying…' : 'Retry FL'}</button
		><button onclick={() => command('restart', '/api/app/restart')} disabled={pending !== null}
			>{pending === 'restart' ? 'Restarting…' : 'Restart app'}</button
		>
	</div>
</section>

{#if $commandError}<div class="alert" role="alert">{$commandError}</div>{/if}

<section class="metrics" aria-label="Runtime metrics">
	<MetricCard
		eyebrow="Runtime"
		value={$runtimeState.phase.replaceAll('_', ' ')}
		detail={`up ${elapsed($runtimeState.startedAtUnixMs)}`}
	/>
	<MetricCard
		eyebrow="FL Studio"
		value={$runtimeState.fl.windowReady ? 'Window ready' : 'Waiting'}
		detail={$runtimeState.fl.pid ? `PID ${$runtimeState.fl.pid}` : 'no process'}
	/>
	<MetricCard
		eyebrow="Gopher"
		value={$runtimeState.fl.gopherReady ? 'Connected' : 'Offline'}
		detail={$runtimeState.fl.gopherToolCount === null
			? 'tools unavailable'
			: `${$runtimeState.fl.gopherToolCount} tools`}
	/>
	<MetricCard
		eyebrow="Scripting"
		value={$runtimeState.app.scriptingConnected ? 'Connected' : 'Waiting'}
		detail={$runtimeState.app.threadId ?? 'no active thread'}
	/>
</section>

<div class="dashboard-grid">
	<section class="panel lifecycle">
		<div class="panel-head">
			<div>
				<p class="kicker">Lifecycle</p>
				<h2>Active bootstrap</h2>
			</div>
			<StatusBadge
				label={$runtimeState.phase}
				tone={$runtimeState.phase === 'ready' ? 'good' : 'warn'}
			/>
		</div>
		<ol>
			{#each [['FL window', $runtimeState.fl.windowReady], ['Gopher target', $runtimeState.fl.gopherReady], ['Application', $runtimeState.app.healthy], ['Scripting bridge', $runtimeState.app.scriptingConnected === true]] as step, index (step[0])}<li
					class:done={step[1]}
				>
					<span>{index + 1}</span>
					<div><strong>{step[0]}</strong><small>{step[1] ? 'ready' : 'pending'}</small></div>
				</li>{/each}
		</ol>
	</section>
	<section class="panel">
		<div class="panel-head">
			<div>
				<p class="kicker">Registered apps</p>
				<h2>Runtime routes</h2>
			</div>
			<a href={resolve('/apps/[appId]', { appId: 'workspace' })}>Open all</a>
		</div>
		<div class="apps-list">
			{#each $registeredApps as app (app.id)}<a
					class="app-row"
					href={resolve('/apps/[appId]', { appId: app.id })}
					><span class="app-icon">FL</span>
					<div><strong>{app.displayName}</strong><code>{app.endpoint}</code></div>
					<StatusBadge
						label={app.healthy ? 'healthy' : 'offline'}
						tone={app.healthy ? 'good' : 'bad'}
					/><span aria-hidden="true">→</span></a
				>{/each}
		</div>
	</section>
	<section class="panel events">
		<div class="panel-head">
			<div>
				<p class="kicker">Telemetry</p>
				<h2>Recent events</h2>
			</div>
			<a href={resolve('/diagnostics')}>Inspect log</a>
		</div>
		<div class="event-list">
			{#each $runtimeEvents.slice(0, 6) as event (event.sequence)}<div class="event-row">
					<time>{new Date(event.timestampUnixMs).toLocaleTimeString([], { hour12: false })}</time
					><span class="severity {event.severity}"></span><code>{event.component}</code><strong
						>{event.event.replaceAll('_', ' ')}</strong
					>
				</div>{/each}
		</div>
	</section>
</div>
