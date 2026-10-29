-- PROPUESTA D-AUTH-10. No ejecutar sin aceptación formal.
-- Depende de 0001 (organization), 0002 (pos_user) y 0003 (role).
-- El secreto de bootstrap vive en configuración de despliegue, nunca en esta tabla.

CREATE TABLE bootstrap_record (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    completed_at TEXT,
    organization_id TEXT,
    administrator_user_id TEXT,
    administrator_role_id TEXT,
    CHECK (
        (completed_at IS NULL AND organization_id IS NULL
            AND administrator_user_id IS NULL AND administrator_role_id IS NULL)
        OR (completed_at IS NOT NULL AND organization_id IS NOT NULL
            AND administrator_user_id IS NOT NULL AND administrator_role_id IS NOT NULL)
    ),
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    FOREIGN KEY (administrator_user_id, organization_id)
        REFERENCES pos_user(id, organization_id),
    FOREIGN KEY (administrator_role_id, organization_id) REFERENCES role(id, organization_id)
) STRICT;

INSERT INTO bootstrap_record (id) VALUES (1);
