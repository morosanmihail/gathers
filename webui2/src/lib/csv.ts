/**
 * Parses up to `maxRows` records of CSV text (RFC 4180: quoted fields may
 * hold commas, newlines and doubled quotes). Enough to show a file's header
 * and a few rows before it is uploaded; the server does the real parsing.
 */
export function parseCsvRows(text: string, maxRows: number): string[][] {
	const rows: string[][] = [];
	let row: string[] = [];
	let field = '';
	let quoted = false;
	let i = text.charCodeAt(0) === 0xfeff ? 1 : 0;
	const endRow = () => {
		row.push(field);
		field = '';
		// Skip blank lines.
		if (row.length > 1 || row[0] !== '') rows.push(row);
		row = [];
	};
	for (; i < text.length && rows.length < maxRows; i++) {
		const c = text[i];
		if (quoted) {
			if (c === '"' && text[i + 1] === '"') { field += '"'; i++; }
			else if (c === '"') quoted = false;
			else field += c;
		} else if (c === '"') quoted = true;
		else if (c === ',') { row.push(field); field = ''; }
		else if (c === '\n' || c === '\r') {
			if (c === '\r' && text[i + 1] === '\n') i++;
			endRow();
		} else field += c;
	}
	if (rows.length < maxRows && (field !== '' || row.length > 0)) endRow();
	return rows.map(r => r.map(f => f.trim()));
}
