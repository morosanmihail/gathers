// Shared hover-tooltip behavior: delayed hide (so moving the pointer from the
// trigger into the tooltip itself doesn't flicker it closed) and viewport-aware
// horizontal placement. Vertical placement varies per tooltip (some flip above,
// some clamp to viewport) so that stays with each component.
export function createHoverTooltip(hideDelayMs = 120) {
	let visible = $state(false);
	let style = $state('');
	let hideTimer: ReturnType<typeof setTimeout>;

	function showEl(el: HTMLElement, position: (el: HTMLElement) => string) {
		clearTimeout(hideTimer);
		visible = true;
		style = position(el);
	}

	function hide() {
		hideTimer = setTimeout(() => { visible = false; }, hideDelayMs);
	}

	function cancelHide() {
		clearTimeout(hideTimer);
	}

	return {
		get visible() { return visible; },
		get style() { return style; },
		showEl,
		hide,
		cancelHide
	};
}

// Left/right placement clamped to stay within the viewport — shared by every
// tooltip that anchors below/above its trigger (price, set-name).
export function clampHorizontal(rect: DOMRect, width: number, margin = 8): string {
	const spaceRight = window.innerWidth - rect.right;
	return spaceRight >= width + margin
		? `left: ${rect.left}px;`
		: `right: ${window.innerWidth - rect.right}px;`;
}
