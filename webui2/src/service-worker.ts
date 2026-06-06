/// <reference no-default-lib="true"/>
/// <reference lib="esnext" />
/// <reference lib="webworker" />

declare const self: ServiceWorkerGlobalScope;

const CACHE = 'gathers-images-v1';

self.addEventListener('install', () => self.skipWaiting());

self.addEventListener('activate', (e) => {
	e.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', (e) => {
	const req = e.request;
	if (req.method !== 'GET') return;
	let url: URL;
	try { url = new URL(req.url); } catch { return; }
	// Same-origin requests (API, app assets) pass through on any domain
	if (url.origin === self.location.origin) return;
	// Only cache image resources
	const dest = req.destination;
	if (dest !== 'image' && dest !== '') return; // '' covers older browsers that don't set destination

	e.respondWith(
		caches.open(CACHE).then(async (cache) => {
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
