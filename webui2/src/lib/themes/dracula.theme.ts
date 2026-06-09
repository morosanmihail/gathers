import type { ThemeDefinition } from './index';

const theme: ThemeDefinition = {
	id: 'dracula',
	label: 'Dracula',
	dot: '#bd93f9',
	vars: {
		'--bg':          '#282a36',
		'--bg2':         '#21222c',
		'--surface':     '#44475a',
		'--surface2':    '#4d5068',
		'--surface3':    '#565970',
		'--border':      '#44475a',
		'--border2':     '#6272a4',
		'--accent':      '#bd93f9',
		'--accent-dim':  '#8a64c8',
		'--accent-glow': '#bd93f933',
		'--accent-text': '#bd93f9',
		'--text':        '#f8f8f2',
		'--text2':       '#6272a4',
		'--text3':       '#44475a',
		'--danger':      '#ff5555',
		'--success':     '#50fa7b',
		'--info':        '#8be9fd',
		'--shadow':      '0 4px 24px #00000088',
		'--shadow-sm':   '0 2px 8px #00000066',
	},
};

export default theme;
