export interface ThemeDefinition {
	id: string;
	label: string;
	/** Accent/preview colour shown in the theme switcher dot. */
	dot: string;
	/** CSS custom property values applied to [data-theme="id"]. */
	vars: Record<string, string>;
}

const modules = import.meta.glob<{ default: ThemeDefinition }>('./*.theme.ts', { eager: true });

export const themes: ThemeDefinition[] = Object.values(modules)
	.map(m => m.default)
	.filter(Boolean)
	.sort((a, b) => a.label.localeCompare(b.label));

/** Inject all theme CSS rules into <head> once at startup. */
export function injectThemeStyles() {
	if (typeof document === 'undefined') return;
	const existing = document.getElementById('gathers-themes');
	if (existing) return;
	const style = document.createElement('style');
	style.id = 'gathers-themes';
	style.textContent = themes
		.map(t => {
			const body = Object.entries(t.vars).map(([k, v]) => `  ${k}: ${v};`).join('\n');
			return `[data-theme="${t.id}"] {\n${body}\n}`;
		})
		.join('\n\n');
	document.head.appendChild(style);
}
