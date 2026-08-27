<script lang="ts">
	import { page } from '$app/state';
	// import StatusBadge from '@ghost/runtime-ui/StatusBadge.svelte';
	import { registeredApps } from '$lib/runtime/client';
	// import { runCommand } from '$lib/runtime/client';
	const app = $derived($registeredApps.find((item) => item.id === page.params.appId));
	let refreshKey = $state(0);
</script>

<svelte:head><title>{app?.displayName ?? 'App'} · Ghost Runtime</title></svelte:head>
<!-- <section class="app-header">
	<div>
		<p class="kicker">Shared app surface</p>
		<h1>{app?.displayName ?? 'Unknown app'}</h1>
		<p>
			The existing application remains isolated behind its runtime route while the shell owns
			lifecycle and navigation.
		</p>
	</div>
	{#if app}<StatusBadge
			label={app.healthy ? 'healthy' : 'offline'}
			tone={app.healthy ? 'good' : 'bad'}
		/>{/if}
</section> -->
{#if app}
	<!-- <div class="app-toolbar">
		<code>{app.uiUrl}</code>
		<div>
			<button class="secondary compact" onclick={() => refreshKey++}>Reload view</button><button
				class="compact"
				onclick={() => runCommand('/api/app/restart')}>Restart process</button
			>
		</div>
	</div> -->
	<div class="app-frame">
		{#key refreshKey}
			<iframe
				title={`${app.displayName} application`}
				src={app.uiUrl}
				sandbox="allow-downloads allow-forms allow-modals allow-popups allow-same-origin allow-scripts"
				allow="clipboard-read; clipboard-write"
			>
			</iframe>
		{/key}
	</div>
{:else}
	<div class="empty">
		<h2>App route not registered</h2>
		<p>This route is available once the Rust runtime publishes its app descriptor.</p>
	</div>
{/if}
