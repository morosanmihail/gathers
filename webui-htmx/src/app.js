// Handle form submission and convert form data to JSON format expected by the API
document.addEventListener('DOMContentLoaded', function() {
    const searchForm = document.getElementById('search-form');

    if (searchForm) {
        searchForm.addEventListener('submit', function(e) {
            e.preventDefault();

            // Convert form data to object
            const formData = new FormData(searchForm);
            const searchParams = {};

            for (let [key, value] of formData.entries()) {
                if (value) {  // Only include non-empty values
                    searchParams[key] = value;
                }
            }

            // Handle color_identities - convert string to array if provided
            if (searchParams.color_identities) {
                const colors = searchParams.color_identities.toUpperCase().split('');
                searchParams.color_identities = colors.filter(color =>
                    ['W', 'U', 'B', 'R', 'G'].includes(color)
                );
            }

            // Handle subtypes and types - convert comma-separated to array
            if (searchParams.subtypes) {
                searchParams.subtypes = searchParams.subtypes.split(',').map(item => item.trim());
            }

            if (searchParams.types) {
                searchParams.types = searchParams.types.split(',').map(item => item.trim());
            }

            // Set default limit and skip values
            limit = parseInt(searchParams.limit || '20');
            skip = 0;

            // Send as JSON to the server
            fetch('/mtg/cards/search?limit='+limit+'&skip='+skip, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify(searchParams)
            })
            .then(response => response.json())
            .then(data => {
                // Render the results using HTMX template
                const resultsContainer = document.getElementById('results-container');
                if (data.length === 0) {
                    resultsContainer.innerHTML = '<div class="no-results">No cards found matching your criteria.</div>';
                } else {
                    renderCardResults(data, resultsContainer);
                }
            })
            .catch(error => {
                console.error('Error searching cards:', error);
                const resultsContainer = document.getElementById('results-container');
                resultsContainer.innerHTML = '<div class="no-results">Error searching cards. Please try again.</div>';
            });
        });
    }
});

/**
 * Render card search results
 * @param {Array} cards - Array of card objects from API
 * @param {HTMLElement} container - DOM element to render into
 */
function renderCardResults(cards, container) {
    let html = '';

    cards.forEach(card => {
        // Format color identity for display
        const colorDisplay = card.color_identity && card.color_identity.length > 0
            ? card.color_identity.join(', ')
            : 'Colorless';

        // Format rarity with proper capitalization
        const rarityDisplay = card.rarity
            ? card.rarity.charAt(0).toUpperCase() + card.rarity.slice(1)
            : '';

        html += `
            <div class="card-result">
                <div class="card-header">
                    <div class="card-name">${card.name}</div>
                    <div class="card-meta">${rarityDisplay} • ${card.set_code || 'Unknown Set'}</div>
                </div>

                <div class="card-details">
                    <div class="detail-item">
                        <span class="detail-label">Colors:</span> ${colorDisplay}
                    </div>
                    <div class="detail-item">
                        <span class="detail-label">Type:</span> ${card.types ? card.types.join(', ') : 'Unknown'}
                    </div>
                    <div class="detail-item">
                        <span class="detail-label">Set:</span> ${card.set_code || 'Unknown'} ${card.set_name ? `(${card.set_name})` : ''}
                    </div>
                    <div class="detail-item">
                        <span class="detail-label">Number:</span> ${card.collector_number || 'N/A'}
                    </div>
                    <div class="detail-item">
                        <span class="detail-label">Artist:</span> ${card.artist || 'Unknown'}
                    </div>
                </div>

                ${card.text ? `
                <div class="detail-item" style="margin-top: 10px; padding-top: 10px; border-top: 1px solid #eee;">
                    <span class="detail-label">Text:</span> ${card.text}
                </div>
                ` : ''}

                ${card.power && card.toughness ? `
                <div class="detail-item">
                    <span class="detail-label">Power/Toughness:</span> ${card.power}/${card.toughness}
                </div>
                ` : ''}
            </div>
        `;
    });

    container.innerHTML = html;
}

// Add event listeners for HTMX responses
document.body.addEventListener('htmx:afterSwap', function(evt) {
    if (evt.detail.target.id === 'results-container') {
        // Any additional processing after results are loaded can go here
        console.log('Card search results loaded');
    }
});
