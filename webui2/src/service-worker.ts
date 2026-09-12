/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />

import { build, files, version } from '$service-worker';

declare const self: ServiceWorkerGlobalScope;

const APP_CACHE = `gathers-app-${version}`;
const IMAGE_CACHE = 'gathers-images-v1';
// Everything the build emits (hashed JS/CSS) plus static/ (manifest, icons, fonts…) —
// precached so the app shell still loads offline / on a flaky connection.
const APP_ASSETS = new Set<string>([...build, ...files]);

self.addEventListener('install', (e) => {
	e.waitUntil(
		caches
			.open(APP_CACHE)
			.then((cache) => cache.addAll([...APP_ASSETS]))
			.then(() => self.skipWaiting())
	);
});

self.addEventListener('activate', (e) => {
	e.waitUntil(
		(async () => {
			const keep = new Set([APP_CACHE, IMAGE_CACHE]);
			for (const key of await caches.keys()) {
				if (!keep.has(key)) await caches.delete(key);
			}
			await self.clients.claim();
		})()
	);
});

self.addEventListener('fetch', (e) => {
	const req = e.request;
	if (req.method !== 'GET') return;
	let url: URL;
	try { url = new URL(req.url); } catch { return; }

	if (url.origin === self.location.origin) {
		// Precached app-shell assets: serve from cache, refresh in the background
		// so a new deploy's assets replace the old ones on the next load.
		if (APP_ASSETS.has(url.pathname)) {
			e.respondWith(
				caches.open(APP_CACHE).then(async (cache) => {
					const hit = await cache.match(url.pathname);
					const refresh = fetch(req)
						.then((res) => { if (res.ok) cache.put(url.pathname, res.clone()); return res; })
						.catch(() => undefined);
					return hit ?? (await refresh) ?? fetch(req);
				})
			);
			return;
		}
		// SPA navigations: fall back to the cached shell when offline so the
		// client-side router still has something to boot from.
		if (req.mode === 'navigate') {
			e.respondWith(
				fetch(req).catch(async () => (await caches.match('/')) ?? fetch(req))
			);
			return;
		}
		// API calls and anything else same-origin: always go to the network —
		// collection/card data shouldn't be served stale from a cache.
		return;
	}

	// Only cache image resources
	const dest = req.destination;
	if (dest !== 'image' && dest !== '') return; // '' covers older browsers that don't set destination

	e.respondWith(
		caches.open(IMAGE_CACHE).then(async (cache) => {
			const hit = await cache.match(req);
			if (hit) return hit;
			const res = await fetch(req);
			const ct = res.headers.get('content-type') ?? '';
			if ((res.ok || res.type === 'opaque') && (ct.startsWith('image/') || res.type === 'opaque')) {
				cache.put(req, res.clone());
			}
			return res;
		})
	);
});
