-- Quantities are read back as i32, but nothing used to stop a row's running
-- total from growing past i32::MAX (or, via edits, going negative); a single
-- such row made its whole collection fail to load. Clamp any existing ones
-- into range and drop rows/entries that end up empty.
UPDATE cards SET
  quantity = max(min(quantity, 2147483647), 0),
  want_quantity = max(min(want_quantity, 2147483647), 0)
WHERE quantity NOT BETWEEN 0 AND 2147483647
   OR want_quantity NOT BETWEEN 0 AND 2147483647;

DELETE FROM cards WHERE quantity = 0 AND want_quantity = 0;

UPDATE purchase_history SET quantity = 2147483647 WHERE quantity > 2147483647;
DELETE FROM purchase_history WHERE quantity <= 0;
UPDATE purchase_history SET price_per_unit = NULL WHERE price_per_unit < 0;
