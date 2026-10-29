-- PROPUESTA D-AUTH-10. No ejecutar sin aceptación formal.
-- Depende de 0002 (installation_identity), 0001 (organization), 0003 (pos_user)
-- y 0004 (role).
--
-- El bootstrap es one-shot por instalación: una fila por installation_id.
-- PENDING puede existir antes de que haya organización; COMPLETED exige que la
-- instalación y la organización pertenezcan entre sí (FK compuesta), y que
-- administrador y rol sean de esa misma organización.
-- El secreto de bootstrap vive en configuración de despliegue, nunca aquí.

CREATE TABLE bootstrap_record (
    installation_id TEXT PRIMARY KEY NOT NULL CHECK (
        instr(installation_id,char(0))=0 AND length(installation_id)=36
        AND substr(installation_id,9,1)='-' AND substr(installation_id,14,1)='-'
        AND substr(installation_id,19,1)='-' AND substr(installation_id,24,1)='-'
        AND length(replace(installation_id,'-',''))=32
        AND replace(installation_id,'-','') NOT GLOB '*[^0-9a-f]*'
    ),
    state TEXT NOT NULL DEFAULT 'PENDING' CHECK (state IN ('PENDING','COMPLETED')),
    completed_at TEXT,
    organization_id TEXT,
    administrator_user_id TEXT,
    administrator_role_id TEXT,
    bootstrap_version INTEGER NOT NULL DEFAULT 1 CHECK (bootstrap_version >= 1),
    updated_at TEXT NOT NULL,
    -- PENDING no admite provisioning parcial; COMPLETED exige todo completo.
    CHECK (
        (state = 'PENDING' AND completed_at IS NULL AND organization_id IS NULL
            AND administrator_user_id IS NULL AND administrator_role_id IS NULL)
        OR (state = 'COMPLETED' AND completed_at IS NOT NULL AND organization_id IS NOT NULL
            AND administrator_user_id IS NOT NULL AND administrator_role_id IS NOT NULL)
    ),
    FOREIGN KEY (installation_id) REFERENCES installation_identity(id),
    -- Ownership compuesta: la organización del bootstrap debe ser la de la
    -- instalación. Con organization_id NULL (PENDING) la FK no se evalúa; la FK
    -- simple anterior garantiza que la instalación existe.
    FOREIGN KEY (installation_id, organization_id)
        REFERENCES installation_identity(id, organization_id),
    FOREIGN KEY (organization_id) REFERENCES organization(id),
    -- Administrador y rol pertenecen a la misma organización.
    FOREIGN KEY (administrator_user_id, organization_id)
        REFERENCES pos_user(id, organization_id),
    FOREIGN KEY (administrator_role_id, organization_id) REFERENCES role(id, organization_id)
) STRICT;
