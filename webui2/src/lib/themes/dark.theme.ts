import type { ThemeDefinition } from './index';

const theme: ThemeDefinition = {
	id: 'dark',
	label: 'Dark',
	dot: '#0d0d12',
	vars: {
		'--bg':          '#0d0d12',
		'--bg2':         '#13131a',
		'--surface':     '#17171f',
		'--surface2':    '#1e1e2a',
		'--surface3':    '#252535',
		'--border':      '#2a283a',
		'--border2':     '#3a3850',
		'--accent':      '#c8974a',
		'--accent-dim':  '#8a6428',
		'--accent-glow': '#c8974a33',
		'--accent-text': '#e0b870',
		'--text':        '#ddd8cc',
		'--text2':       '#a09888',
		'--text3':       '#5a5448',
		'--danger':      '#e05252',
		'--success':     '#52c070',
		'--info':        '#5090d8',
		'--shadow':      '0 4px 24px #00000088',
		'--shadow-sm':   '0 2px 8px #00000066',
	},
};

export default theme;
