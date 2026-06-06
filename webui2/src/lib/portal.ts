// Svelte action: moves element to document.body so it escapes
// any parent transform/overflow that would mis-position fixed children.
export function portal(el: HTMLElement) {
	document.body.appendChild(el);
	return {
		destroy() { el.remove(); }
	};
}
