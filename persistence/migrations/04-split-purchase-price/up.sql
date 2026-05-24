ALTER TABLE purchase_history ADD COLUMN normal_price_per_unit REAL;
ALTER TABLE purchase_history ADD COLUMN foil_price_per_unit REAL;
UPDATE purchase_history SET
  normal_price_per_unit = CASE WHEN quantity > 0 THEN price_per_unit ELSE NULL END,
  foil_price_per_unit   = CASE WHEN foil_quantity > 0 THEN price_per_unit ELSE NULL END;
ALTER TABLE purchase_history DROP COLUMN price_per_unit;
