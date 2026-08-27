<script lang="ts">
	let { eyebrow, value, detail }: { eyebrow: string; value: string; detail: string } = $props();
</script>
<article class="metric">
	<p>{eyebrow}</p><strong>{value}</strong><small>{detail}</small>
</article>
<style>
	.metric { min-width:0;border:1px solid var(--line);border-radius:var(--radius);background:var(--panel);padding:1rem }
	p { margin:0 0 .65rem;color:var(--muted);font:600 .69rem/1 var(--sans);letter-spacing:.1em;text-transform:uppercase }
	strong { display:block;overflow:hidden;text-overflow:ellipsis;color:var(--text);font:500 1.45rem/1.2 var(--sans);white-space:nowrap }
	small { display:block;margin-top:.45rem;color:var(--faint);font:400 .77rem/1.4 var(--mono) }
</style>
