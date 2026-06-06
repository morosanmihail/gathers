// In-memory URL cache — avoids repeated cardImageUrl() computation and prevents
// placeholder flicker on re-renders. Blob-fetching is only attempted for Scryfall
// (which supports CORS); all other CDNs are cached as direct URLs and loaded by
// the browser <img> tag (which bypasses CORS) with normal HTTP caching.
const cache = new Map<string, string>();

const BLOB_FETCH_HOSTS = new Set(['api.scryfall.com']);

// Synchronous lookup — returns cached value instantly, or '' if not yet fetched.
// Use as initial $state value in components to skip the placeholder flash.
export function syncCachedImageUrl(url: string): string {
	return cache.get(url) ?? '';
}

export async function cachedImageUrl(url: string): Promise<string> {
	if (!url) return '';
	if (cache.has(url)) return cache.get(url)!;

	const host = (() => { try { return new URL(url).hostname; } catch { return ''; } })();

	if (BLOB_FETCH_HOSTS.has(host)) {
		try {
			const res = await fetch(url, { mode: 'cors' });
			if (res.ok) {
				const objectUrl = URL.createObjectURL(await res.blob());
				cache.set(url, objectUrl);
				return objectUrl;
			}
		} catch {
			// CORS unexpectedly failed — fall through to direct URL
		}
	}

	// For all other hosts: cache the direct URL and let the browser handle HTTP caching
	cache.set(url, url);
	return url;
}
