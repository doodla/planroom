-- ship now requires a tracking number (see hasTrackingNumber).
ALTER TABLE orders ADD COLUMN tracking_number text NOT NULL DEFAULT '';
