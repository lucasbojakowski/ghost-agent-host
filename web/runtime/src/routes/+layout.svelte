<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { onMount } from 'svelte';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import StatusBadge from '@ghost/runtime-ui/StatusBadge.svelte';
	import { connectRuntime, connectionState, runtimeState } from '$lib/runtime/client';

	let { children } = $props();
	onMount(() => {
		let dispose = () => {};
		void connectRuntime().then((next) => (dispose = next));
		return () => dispose();
	});
	const nav = [
		{ href: '/', label: 'Overview', icon: '⌁' },
		{ href: '/apps/workspace', label: 'Apps', icon: '◇' },
		{ href: '/diagnostics', label: 'Diagnostics', icon: '≋' },
		{ href: '/settings', label: 'Settings', icon: '⚙' }
	] as const;
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>
<div class="shell">
	<aside class="rail">
		<a class="brand" href={resolve('/')} aria-label="Ghost runtime home"
			><span>G</span><b>Ghost</b><small>Runtime</small></a
		>
		<nav aria-label="Primary navigation">
			{#each nav as item (item.href)}<a
					href={resolve(item.href)}
					aria-current={page.url.pathname === item.href ||
					(item.href !== '/' && page.url.pathname.startsWith(item.href))
						? 'page'
						: undefined}><i aria-hidden="true">{item.icon}</i><span>{item.label}</span></a
				>{/each}
		</nav>
		<div class="rail-foot">
			<StatusBadge
				label={$connectionState}
				tone={$connectionState === 'live' || $connectionState === 'mock' ? 'good' : 'warn'}
			/><code>{$runtimeState.sessionId.slice(0, 13)}</code>
		</div>
	</aside>
	<div class="stage" class:app-stage={page.url.pathname.includes('/apps')}>
		{#if !page.url.pathname.includes('/apps')}
			<header class="topbar">
				<div>
					<span class="crumb">Runtime</span><span class="slash">/</span><strong
						>{nav.find(
							(item) =>
								page.url.pathname === item.href ||
								(item.href !== '/' && page.url.pathname.startsWith(item.href))
						)?.label ?? 'Overview'}</strong
					>
				</div>
				<StatusBadge
					label={$runtimeState.phase.replaceAll('_', ' ')}
					tone={$runtimeState.phase === 'ready'
						? 'good'
						: $runtimeState.phase === 'failed'
							? 'bad'
							: 'warn'}
				/>
			</header>
			<main class="main-content-runtime" id="main-content">{@render children()}</main>
		{:else}
			<main class="main-content-app" id="main-content">{@render children()}</main>
		{/if}
	</div>
</div>
<div class="sr-live" aria-live="polite">Runtime connection {$connectionState}</div>
