// Runs before first paint so the page never flashes the wrong theme.
//
// Deliberately a separate file rather than an inline <script> in app.html:
// SvelteKit's CSP hash mode only hashes the scripts it generates itself, so an
// inline script here would be blocked by the script-src policy. Served from the
// same origin, it is covered by 'self'.
(function () {
	try {
		var stored = localStorage.getItem('hako-theme');
		var theme =
			stored === 'light' || stored === 'dark'
				? stored
				: window.matchMedia('(prefers-color-scheme: dark)').matches
					? 'dark'
					: 'light';
		document.documentElement.dataset.theme = theme;
	} catch {
		/* private mode / no storage — keep the default */
	}
})();
