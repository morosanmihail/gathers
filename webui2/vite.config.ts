import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import { appVersion } from './version.js';

export default defineConfig({
	plugins: [sveltekit()],
	define: { __APP_VERSION__: JSON.stringify(appVersion) },
	server: {
		port: 5173,
		strictPort: true,
		proxy: {
			'/api': 'http://localhost:5234'
		}
	}
});
