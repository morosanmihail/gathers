import type { ThemeDefinition } from './index';

const theme: ThemeDefinition = {
	id: 'light',
	label: 'Light',
	dot: '#f0ede6',
	vars: {
		'--bg':          '#f4f2ed',
		'--bg2':         '#ece9e2',
		'--surface':     '#ffffff',
		'--surface2':    '#f7f5f0',
		'--surface3':    '#edeae2',
		'--border':      '#d8d4c8',
		'--border2':     '#c4bfb0',
		'--accent':      '#a0721a',
		'--accent-dim':  '#c8942a',
		'--accent-glow': '#a0721a22',
		'--accent-text': '#7a5010',
		'--text':        '#2a2420',
		'--text2':       '#6a6058',
		'--text3':       '#a09880',
		'--danger':      '#c83030',
		'--success':     '#2a8040',
		'--info':        '#2060b8',
		'--shadow':      '0 4px 24px #00000022',
		'--shadow-sm':   '0 2px 8px #00000018',
	},
};

export default theme;
