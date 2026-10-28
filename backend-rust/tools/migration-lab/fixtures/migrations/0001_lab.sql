-- Solo laboratorio: no representa el dominio F1-10 ni el baseline Java.
CREATE TABLE lab_parent (
    id INTEGER PRIMARY KEY,
    label TEXT NOT NULL
) STRICT;

CREATE TABLE lab_child (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER NOT NULL REFERENCES lab_parent(id),
    quantity INTEGER NOT NULL CHECK (quantity >= 0)
) STRICT;
