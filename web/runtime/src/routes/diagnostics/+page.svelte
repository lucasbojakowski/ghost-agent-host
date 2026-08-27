<script lang="ts">
	import { runtimeEvents, runtimeState } from '$lib/runtime/client';
</script>

<svelte:head><title>Diagnostics · Ghost Runtime</title></svelte:head>
<section class="page-title">
	<div>
		<p class="kicker">Diagnostics</p>
		<h1>Runtime event journal</h1>
		<p>Ordered lifecycle telemetry streamed over the runtime WebSocket.</p>
	</div>
</section>
<section class="panel diagnostic-summary">
	<div><span>Session</span><code>{$runtimeState.sessionId}</code></div>
	<div><span>Events in memory</span><strong>{$runtimeEvents.length}</strong></div>
	<div>
		<span>Last update</span><time>{new Date($runtimeState.updatedAtUnixMs).toLocaleString()}</time>
	</div>
</section>
<section class="panel event-table" aria-label="Runtime event log">
	<div class="event-table-head">
		<span>Time</span><span>Source</span><span>Event</span><span>Payload</span>
	</div>
	{#each $runtimeEvents as event (event.sequence)}<article>
			<time>{new Date(event.timestampUnixMs).toLocaleTimeString([], { hour12: false })}</time><code
				>{event.component}</code
			><strong><span class="severity {event.severity}"></span>{event.event}</strong>
			<pre>{JSON.stringify(event.data)}</pre>
		</article>{/each}
</section>
