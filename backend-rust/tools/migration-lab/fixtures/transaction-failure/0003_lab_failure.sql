-- La tabla y la fila deben desaparecer cuando SQLx revierta la migración.
CREATE TABLE lab_partial (id INTEGER PRIMARY KEY) STRICT;
INSERT INTO lab_partial(id) VALUES (1);
INSERT INTO lab_missing(id) VALUES (1);
