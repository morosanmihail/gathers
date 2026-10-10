export function fmtDate(iso: string): string {
	try { return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); }
	catch { return iso; }
}

const RETAILER_LABELS: Record<string, string> = {
	cardkingdom: 'Card Kingdom',
	cardmarket: 'Cardmarket',
	cardsphere: 'Cardsphere',
	manapool: 'Mana Pool',
	tcgplayer: 'TCGplayer',
	raw: 'Ungraded',
	graded_psa10: 'PSA 10',
	graded_psa9: 'PSA 9',
};

/** Display name for a price source key like `cardkingdom` or `graded_psa10`;
 *  unknown keys are title-cased with `_`/`-` as spaces. */
export function retailerLabel(key: string): string {
	return RETAILER_LABELS[key.toLowerCase()]
		?? key.replace(/[_-]+/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
}
