import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	// Consult https://svelte.dev/docs/kit/integrations
	// for more information about preprocessors
	preprocess: vitePreprocess(),

	kit: {
		// Emits a <meta http-equiv="content-security-policy"> into the prerendered
		// HTML with a sha256 of SvelteKit's own inline bootstrap script, so the
		// page can run under script-src without 'unsafe-inline'. Hash mode (not
		// nonce) because adapter-static output is served as plain files by Actix,
		// with no server to mint a per-request nonce.
		//
		// This only covers scripts SvelteKit generates. Anything hand-written in
		// app.html must be an external file — see static/theme.js.
		csp: {
			mode: 'hash',
			directives: {
				'script-src': ['self']
			}
		},

		// adapter-auto only supports some environments, see https://svelte.dev/docs/kit/adapter-auto for a list.
		// If your environment is not supported, or you settled on a specific environment, switch out the adapter.
		// See https://svelte.dev/docs/kit/adapters for more information about adapters.
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html',
			precompress: false,
			strict: true
		})
	}
};

export default config;
