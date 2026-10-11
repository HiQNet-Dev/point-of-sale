-- Operational catalog discovery is separately grantable from catalog administration.
-- Existing role grants are intentionally unchanged; fresh bootstrap receives the full catalog.
INSERT INTO permissions (code, description) VALUES
    ('pos.catalog.read', 'Discover organization catalog for POS operations in an authorized branch');
