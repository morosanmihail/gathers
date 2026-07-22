export function fmtDate(iso: string): string {
	try { return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); }
	catch { return iso; }
}
