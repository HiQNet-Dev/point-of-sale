-- El CREATE y el UPDATE deben revertirse junto al fallo SQLSTATE 42P01.
CREATE TABLE lab_partial (id INTEGER PRIMARY KEY);
UPDATE lab_record SET payload = 'must-rollback';
INSERT INTO lab_missing_table (id) VALUES (1);
