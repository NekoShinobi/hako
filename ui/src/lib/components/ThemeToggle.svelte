<script lang="ts">
	/*
	 * The active theme lives on <html data-theme>, set by the inline script in
	 * app.html before first paint. This button only flips that attribute, and CSS
	 * (not state) decides which icon shows — so the markup is identical on the
	 * server and after hydration, with no icon flash.
	 */
	function toggle() {
		const root = document.documentElement;
		const next = root.dataset.theme === 'dark' ? 'light' : 'dark';
		root.dataset.theme = next;
		try {
			localStorage.setItem('hako-theme', next);
		} catch {
			// Private mode or storage disabled — the choice just won't persist.
		}
	}
</script>

<button
	type="button"
	class="theme-toggle"
	onclick={toggle}
	title="Toggle light / dark theme"
	aria-label="Toggle light / dark theme"
>
	<!-- Shown in light mode: click for dark. -->
	<svg
		class="icon-moon"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
	</svg>
	<!-- Shown in dark mode: click for light. -->
	<svg
		class="icon-sun"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="4.5" />
		<path
			d="M12 2v2M12 20v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M2 12h2M20 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"
		/>
	</svg>
</button>

<style>
	.theme-toggle {
		position: fixed;
		top: 1rem;
		right: 1rem;
		z-index: 20;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 2.5rem;
		height: 2.5rem;
		border: 2px solid var(--color-border-subtle);
		border-radius: 9999px;
		background-color: var(--color-bg-subtle);
		color: var(--color-text-muted);
		cursor: pointer;
		transition:
			background-color 150ms ease,
			border-color 150ms ease,
			color 150ms ease;
	}

	.theme-toggle:hover {
		border-color: var(--color-text-muted);
		color: var(--color-text-dark);
	}

	.theme-toggle:focus-visible {
		outline: 2px solid var(--color-text-dark);
		outline-offset: 2px;
	}

	.theme-toggle svg {
		width: 1.15rem;
		height: 1.15rem;
	}

	.icon-sun {
		display: none;
	}

	:global(:root[data-theme='dark']) .theme-toggle .icon-moon {
		display: none;
	}

	:global(:root[data-theme='dark']) .theme-toggle .icon-sun {
		display: block;
	}

	@media (prefers-reduced-motion: reduce) {
		.theme-toggle {
			transition: none;
		}
	}
</style>
