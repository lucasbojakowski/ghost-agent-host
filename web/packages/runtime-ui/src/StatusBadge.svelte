<script lang="ts">
	let { label, tone = 'neutral' }: { label: string; tone?: 'good' | 'warn' | 'bad' | 'neutral' } = $props();
</script>

<span class="badge {tone}"><span class="dot" aria-hidden="true"></span>{label}</span>

<style>
	.badge { display:inline-flex;align-items:center;gap:.45rem;border:1px solid var(--line);border-radius:999px;padding:.28rem .62rem;color:var(--muted);font:600 .72rem/1 var(--sans);letter-spacing:.04em;text-transform:uppercase }
	.dot { width:.42rem;height:.42rem;border-radius:50%;background:#77808e;box-shadow:0 0 0 .18rem color-mix(in srgb,#77808e 15%,transparent) }
	.good .dot { background:var(--good);box-shadow:0 0 0 .18rem color-mix(in srgb,var(--good) 15%,transparent) }
	.warn .dot { background:var(--warn) }.bad .dot { background:var(--bad) }
</style>
