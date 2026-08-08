<script lang="ts">
	import { onMount, setContext } from 'svelte';
	import { fade } from 'svelte/transition';
	import { prefersReducedMotion } from 'svelte/motion';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';

	const { children } = $props();

	// Fetch config from backend
	let fileExpiryMinutes = $state(180); // Default fallback
	let maxFileSizeMb = $state(100); // Default fallback
	// Server is running on the published placeholder ENCRYPTION_KEY.
	let usingDefaultKey = $state(false);
	async function fetchConfig() {
		try {
			const response = await fetch('/config');
			if (response.ok) {
				const data = await response.json();
				fileExpiryMinutes = data.file_expiry_minutes;
				maxFileSizeMb = data.max_file_size_mb;
				usingDefaultKey = data.using_default_key === true;
			}
		} catch (error) {
			console.error('Failed to fetch config:', error);
		}
	}

	onMount(() => {
		fetchConfig();
	});

	// Format expiry time
	const expiryText = $derived(
		fileExpiryMinutes >= 60
			? `${Math.floor(fileExpiryMinutes / 60)} hour${Math.floor(fileExpiryMinutes / 60) !== 1 ? 's' : ''}`
			: `${fileExpiryMinutes} minutes`
	);

	// Random character image (1-20)
	const characterNum = Math.floor(Math.random() * 20) + 1;

	// Cross-page fade. The incoming page waits out the outgoing one so the two
	// never fight over opacity; both are stacked in the same grid cell (see the
	// .page-fade rules below) so the overlap doesn't shove the footer around.
	const fadeOut = $derived(prefersReducedMotion.current ? 0 : 120);
	const fadeIn = $derived(prefersReducedMotion.current ? 0 : 180);

	// Provide context to child components
	setContext('fileExpiryMinutes', () => fileExpiryMinutes);
	setContext('maxFileSizeMb', () => maxFileSizeMb);
	setContext('expiryText', () => expiryText);
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

<ThemeToggle />

<div class="flex min-h-screen flex-col" style="background-color: var(--color-bg)">
	{#if usingDefaultKey}
		<div
			role="alert"
			class="w-full border-b-2 px-4 py-3 text-center text-sm"
			style="background-color: var(--color-error-bg); border-color: var(--color-error); color: var(--color-text-error);"
		>
			<strong class="font-bold">This server is using the default encryption key.</strong>
			Uploads are not meaningfully encrypted — anyone with the key from the public repository can read
			them. Don't upload anything private.
			<span class="mt-1 block opacity-90">
				Operator: set <code class="font-mono">ENCRYPTION_KEY</code> to the output of
				<code class="font-mono">openssl rand -base64 24</code> and restart.
			</span>
		</div>
	{/if}

	<main class="flex-1 p-8">
		<div class="mx-auto w-full max-w-2xl pt-16 text-center">
			<h1 class="mb-2 text-6xl font-bold" style="color: var(--color-text-dark)">Hako~</h1>
			<p class="mb-2 text-xl" style="color: var(--color-text-muted); font-weight: 300;">箱</p>
			<p class="mb-8" style="color: var(--color-text-muted)">
				Max {maxFileSizeMb} MiB. Files are stored encrypted and deleted after {expiryText}.
			</p>

			<div class="page-fade">
				{#key page.url.pathname}
					<div
						class="page-fade-item"
						in:fade={{ duration: fadeIn, delay: fadeOut }}
						out:fade={{ duration: fadeOut }}
					>
						{@render children()}
					</div>
				{/key}
			</div>
		</div>
	</main>

	<!-- Character image in bottom right -->
	<img
		src="/img/{characterNum}"
		alt=""
		class="pointer-events-none fixed right-4 bottom-0 z-10 h-16 md:h-20 lg:h-24"
		onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = 'none')}
	/>

	<footer class="py-6 text-center text-sm" style="color: var(--color-text-muted)">
		<div class="space-x-4">
			<a href={resolve('/')} class="hover:opacity-70">箱</a>
			<span>|</span>
			<a href={resolve('/api')} class="hover:opacity-70">API</a>
			<span>|</span>
			<a href={resolve('/faq')} class="hover:opacity-70">FAQ</a>
			<span>|</span>
			<a
				href="https://github.com/NekoShinobi/hako"
				class="hover:opacity-70"
				rel="noopener noreferrer">GitHub</a
			>
		</div>
	</footer>
</div>

<style>
	/* Old and new page share one grid cell during the crossfade, so the page
	   never grows to the height of both at once. */
	.page-fade {
		display: grid;
	}

	.page-fade-item {
		grid-area: 1 / 1;
	}
</style>
