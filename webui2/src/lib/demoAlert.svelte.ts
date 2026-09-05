// Standalone reactive store for the "disabled in demo mode" toast.
// Kept out of state.svelte.ts to avoid a circular import with api.ts,
// which needs to trigger this from its shared fetchJSON error handling.

let visible = $state(false);
let timer: ReturnType<typeof setTimeout> | undefined;

export const demoAlert = {
	get visible() {
		return visible;
	},
	show() {
		visible = true;
		clearTimeout(timer);
		timer = setTimeout(() => {
			visible = false;
		}, 3000);
	},
	dismiss() {
		clearTimeout(timer);
		visible = false;
	}
};
