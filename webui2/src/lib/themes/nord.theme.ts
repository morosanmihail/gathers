import type { ThemeDefinition } from './index';

const theme: ThemeDefinition = {
	id: 'nord',
	label: 'Nord',
	dot: '#88c0d0',
	vars: {
		'--bg':          '#2e3440',
		'--bg2':         '#292e39',
		'--surface':     '#3b4252',
		'--surface2':    '#434c5e',
		'--surface3':    '#4c566a',
		'--border':      '#434c5e',
		'--border2':     '#4c566a',
		'--accent':      '#88c0d0',
		'--accent-dim':  '#5e8e9e',
		'--accent-glow': '#88c0d033',
		'--accent-text': '#88c0d0',
		'--text':        '#eceff4',
		'--text2':       '#d8dee9',
		'--text3':       '#4c566a',
		'--danger':      '#bf616a',
		'--success':     '#a3be8c',
		'--info':        '#81a1c1',
		'--shadow':      '0 4px 24px #00000088',
		'--shadow-sm':   '0 2px 8px #00000066',
	},
};

export default theme;
