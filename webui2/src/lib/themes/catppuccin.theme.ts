import type { ThemeDefinition } from './index';

const theme: ThemeDefinition = {
	id: 'catppuccin',
	label: 'Catppuccin',
	dot: '#cba6f7',
	vars: {
		'--bg':          '#1e1e2e',
		'--bg2':         '#181825',
		'--surface':     '#313244',
		'--surface2':    '#45475a',
		'--surface3':    '#585b70',
		'--border':      '#45475a',
		'--border2':     '#6c7086',
		'--accent':      '#cba6f7',
		'--accent-dim':  '#9a74c4',
		'--accent-glow': '#cba6f733',
		'--accent-text': '#cba6f7',
		'--text':        '#cdd6f4',
		'--text2':       '#a6adc8',
		'--text3':       '#7f849c',
		'--danger':      '#f38ba8',
		'--success':     '#a6e3a1',
		'--info':        '#89b4fa',
		'--shadow':      '0 4px 24px #00000088',
		'--shadow-sm':   '0 2px 8px #00000066',
	},
};

export default theme;
