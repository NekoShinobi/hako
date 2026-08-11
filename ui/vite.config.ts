import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// On the host the API is a sibling process on localhost; under `just up-dev`
// it is a separate container, where localhost would be the Vite container
// itself. compose.dev.yml sets API_URL accordingly.
const apiTarget = process.env.API_URL ?? `http://localhost:${process.env.PORT ?? '9090'}`;

// Polling is required where inotify does not cross the mount (the `just up-dev`
// container, Docker Desktop, a network filesystem) and is wasted CPU everywhere
// else, so it is opt-in rather than always on.
const usePolling = process.env.VITE_USE_POLLING === 'true';

// The backend registers exactly these four routes and then serves ./static as
// a catch-all (see App::new() in src/main.rs). The UI calls them with relative
// paths — `fetch('/config')` — so without this block the dev server answers
// them itself and every request 404s against SvelteKit's router. Anything not
// listed here stays with Vite, which is what serves the SvelteKit routes.
const apiRoutes = ['/config', '/upload', '/file', '/img'];

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		watch: {
			usePolling,
			interval: 250,
			// `just build-ui` writes into these, and without the exclusion the dev
			// server fires a full reload for every file the build emits.
			ignored: ['**/build/**', '**/.svelte-kit/output/**', '**/.svelte-kit/generated/**']
		},
		proxy: Object.fromEntries(
			apiRoutes.map((route) => [
				route,
				{
					target: apiTarget,
					// Not changeOrigin: the backend is same-origin in production, and
					// forwarding the original Host keeps dev behaviour identical.
					changeOrigin: false
				}
			])
		)
	}
});
