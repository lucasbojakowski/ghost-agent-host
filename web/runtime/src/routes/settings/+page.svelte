<script lang="ts">
	import { connectionState, runtimeState } from '$lib/runtime/client';
</script>

<svelte:head><title>Settings · Ghost Runtime</title></svelte:head>
<section class="page-title">
	<div>
		<p class="kicker">Settings</p>
		<h1>Runtime preferences</h1>
		<p>System-owned options only. App domain settings stay inside each registered application.</p>
	</div>
</section>
<div class="settings-grid">
	<section class="panel">
		<div class="panel-head">
			<div>
				<p class="kicker">Transport</p>
				<h2>WebSocket first</h2>
			</div>
		</div>
		<dl>
			<div>
				<dt>Connection</dt>
				<dd>{$connectionState}</dd>
			</div>
			<div>
				<dt>Fallback</dt>
				<dd>HTTP snapshot</dd>
			</div>
			<div>
				<dt>Visibility policy</dt>
				<dd>Pause and reconcile</dd>
			</div>
		</dl>
	</section>
	<section class="panel">
		<div class="panel-head">
			<div>
				<p class="kicker">Bootstrap</p>
				<h2>Active mode</h2>
			</div>
		</div>
		<dl>
			<div>
				<dt>FL debug port</dt>
				<dd>{$runtimeState.fl.debugPort}</dd>
			</div>
			<div>
				<dt>Gopher target</dt>
				<dd>{$runtimeState.fl.gopherTarget ?? 'unresolved'}</dd>
			</div>
			<div>
				<dt>App profile</dt>
				<dd>{$runtimeState.app.profile}</dd>
			</div>
		</dl>
	</section>
</div>
