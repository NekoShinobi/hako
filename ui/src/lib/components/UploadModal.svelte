<script lang="ts">
	type Status = 'uploading' | 'success' | 'error';

	const {
		status,
		progress,
		fileName,
		fileSize,
		url = '',
		error = '',
		onclose,
		oncancel
	}: {
		status: Status;
		progress: number;
		fileName: string;
		fileSize: string;
		url?: string;
		error?: string;
		onclose: () => void;
		oncancel: () => void;
	} = $props();

	let copied = $state(false);
	let primaryButton: HTMLButtonElement | undefined = $state();

	// Bytes are on the wire, but the server is still encrypting and storing.
	const finishing = $derived(status === 'uploading' && progress >= 100);

	// This component is conditionally rendered, so onMount would never fire —
	// $effect is the reliable hook here. Re-runs when the button swaps between
	// states, which keeps focus on whatever action is now primary.
	$effect(() => {
		primaryButton?.focus();
	});

	async function copyLink() {
		try {
			await navigator.clipboard.writeText(url);
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 2000);
		} catch (e) {
			console.error('Failed to copy:', e);
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key !== 'Escape') return;
		if (status === 'uploading') oncancel();
		else onclose();
	}

	function handleBackdropClick() {
		// A stray click must not abandon an upload in flight.
		if (status !== 'uploading') onclose();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- The dialog itself carries the keyboard affordances; the backdrop is a
     pointer-only convenience, so it needs no role of its own. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onclick={handleBackdropClick}>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<div
		class="card accent-green modal"
		role="dialog"
		aria-modal="true"
		aria-labelledby="upload-modal-title"
		tabindex="-1"
		onclick={(event) => event.stopPropagation()}
	>
		<h2 id="upload-modal-title" class="text-xl font-semibold">
			{#if status === 'uploading'}
				{finishing ? 'Finishing up…' : 'Uploading…'}
			{:else if status === 'success'}
				Upload complete
			{:else}
				Upload failed
			{/if}
		</h2>

		<p class="file-line" title={fileName}>{fileName} · {fileSize}</p>

		{#if status === 'uploading'}
			<div
				class="track"
				role="progressbar"
				aria-valuemin="0"
				aria-valuemax="100"
				aria-valuenow={progress}
				aria-label="Upload progress"
			>
				<div class="fill" class:pulsing={finishing} style="width: {progress}%"></div>
			</div>
			<div class="row">
				<span class="text-sm">{finishing ? 'Encrypting and storing' : `${progress}%`}</span>
				<button bind:this={primaryButton} type="button" class="btn btn-ghost" onclick={oncancel}>
					Cancel
				</button>
			</div>
		{:else if status === 'success'}
			<div class="track"><div class="fill" style="width: 100%"></div></div>
			<!-- Points at the backend's file-download endpoint, not a SvelteKit route, so resolve() does not apply. -->
			<!-- eslint-disable svelte/no-navigation-without-resolve -->
			<a class="link" href={url} target="_blank" rel="noopener noreferrer">{url}</a>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
			<div class="row">
				<button bind:this={primaryButton} type="button" class="btn btn-solid" onclick={copyLink}>
					{copied ? 'Copied!' : 'Copy link'}
				</button>
				<button type="button" class="btn btn-ghost" onclick={onclose}>Close</button>
			</div>
		{:else}
			<p class="error">{error}</p>
			<div class="row">
				<button bind:this={primaryButton} type="button" class="btn btn-solid" onclick={onclose}>
					Close
				</button>
			</div>
		{/if}
	</div>
</div>

<style>
	.backdrop {
		position: fixed;
		inset: 0;
		z-index: 50;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1rem;
		background-color: rgb(0 0 0 / 0.45);
	}

	.modal {
		width: 100%;
		max-width: 26rem;
		padding: 1.5rem;
		border-width: 2px;
		border-style: solid;
		border-radius: 0.75rem;
		text-align: left;
		box-shadow: 0 10px 30px rgb(0 0 0 / 0.25);
	}

	.file-line {
		margin-top: 0.25rem;
		overflow: hidden;
		font-size: 0.875rem;
		text-overflow: ellipsis;
		white-space: nowrap;
		opacity: 0.85;
	}

	.track {
		margin-top: 1rem;
		height: 0.5rem;
		overflow: hidden;
		border-radius: 9999px;
		background-color: var(--card-code-bg);
	}

	.fill {
		height: 100%;
		border-radius: 9999px;
		background-color: var(--card-heading);
		transition: width 150ms ease;
	}

	.pulsing {
		animation: pulse 1.2s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.55;
		}
	}

	.row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		margin-top: 1rem;
	}

	.link {
		display: block;
		margin-top: 0.75rem;
		padding: 0.5rem 0.75rem;
		border-radius: 0.375rem;
		background-color: var(--card-code-bg);
		color: var(--card-code-text);
		font-size: 0.875rem;
		word-break: break-all;
	}

	.link:hover {
		text-decoration: underline;
	}

	.error {
		margin-top: 0.75rem;
		font-size: 0.875rem;
	}

	.btn {
		padding: 0.4rem 0.9rem;
		border: 2px solid var(--card-heading);
		border-radius: 0.375rem;
		font-size: 0.875rem;
		font-family: inherit;
		cursor: pointer;
		transition: opacity 150ms ease;
	}

	.btn:hover {
		opacity: 0.85;
	}

	.btn:focus-visible {
		outline: 2px solid var(--card-heading);
		outline-offset: 2px;
	}

	.btn-solid {
		background-color: var(--card-heading);
		color: var(--card-code-bg);
	}

	.btn-ghost {
		background-color: transparent;
		color: var(--card-heading);
	}

	@media (prefers-reduced-motion: reduce) {
		.fill {
			transition: none;
		}

		.pulsing {
			animation: none;
		}
	}
</style>
