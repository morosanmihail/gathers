-- Purchase prices were recorded without a currency; everything recorded so
-- far is assumed to be US dollars.
ALTER TABLE purchase_history ADD COLUMN currency TEXT NOT NULL DEFAULT 'USD';
