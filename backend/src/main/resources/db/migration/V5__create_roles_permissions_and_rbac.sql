-- RBAC foundation: system permission catalog, organization-scoped roles,
-- role->permission grants and user->organization role assignments.
--
-- V1-V4 are published and immutable; this migration is additive.
--
-- IMPORTANT (tenant enforcement at the database level): a User of organization A
-- can never be assigned a Role of organization B. This is enforced with
-- composite foreign keys: UNIQUE (id, organization_id) superkeys are added on
-- users and roles purely to enable those FKs (id is already the PK; the
-- composite unique index is redundant-but-required for the FK definition). Java
-- validation is NOT the only barrier.

CREATE TABLE permissions (
    code        VARCHAR(64)  NOT NULL PRIMARY KEY,
    description VARCHAR(255) NOT NULL CHECK (length(trim(description)) > 0)
);

CREATE TABLE roles (
    id              UUID         NOT NULL PRIMARY KEY,
    organization_id UUID         NOT NULL REFERENCES organizations (id),
    code            VARCHAR(64)  NOT NULL,
    name            VARCHAR(120) NOT NULL CHECK (length(trim(name)) > 0),
    status          VARCHAR(16)  NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE')),
    version         BIGINT       NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ  NOT NULL,
    updated_at      TIMESTAMPTZ  NOT NULL,
    CONSTRAINT roles_organization_code_unique UNIQUE (organization_id, code),
    CONSTRAINT roles_code_format CHECK (code ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    CONSTRAINT roles_id_organization_unique UNIQUE (id, organization_id)
);

CREATE TABLE role_permissions (
    role_id         UUID        NOT NULL,
    permission_code VARCHAR(64) NOT NULL,
    CONSTRAINT role_permissions_pk PRIMARY KEY (role_id, permission_code),
    CONSTRAINT role_permissions_role_fk
        FOREIGN KEY (role_id) REFERENCES roles (id),
    CONSTRAINT role_permissions_permission_fk
        FOREIGN KEY (permission_code) REFERENCES permissions (code)
);

-- Unique (id, organization_id) on users: enabler for the composite FK below.
ALTER TABLE users
    ADD CONSTRAINT users_id_organization_unique UNIQUE (id, organization_id);

CREATE TABLE user_organization_roles (
    organization_id UUID        NOT NULL,
    user_id         UUID        NOT NULL,
    role_id         UUID        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL,
    CONSTRAINT user_organization_roles_pk PRIMARY KEY (organization_id, user_id, role_id),
    CONSTRAINT user_organization_roles_user_fk
        FOREIGN KEY (user_id, organization_id) REFERENCES users (id, organization_id),
    CONSTRAINT user_organization_roles_role_fk
        FOREIGN KEY (role_id, organization_id) REFERENCES roles (id, organization_id)
);

-- System permission catalog (created by migration; not a client CRUD).
INSERT INTO permissions (code, description) VALUES
    ('organization.read',    'Read organization details'),
    ('organization.update',  'Update organization details'),
    ('branch.read',          'List and read branches'),
    ('branch.create',        'Create branches'),
    ('branch.update',        'Update branches'),
    ('user.read',            'List and read users'),
    ('user.create',          'Create users'),
    ('user.update',          'Update users'),
    ('permission.read',      'List the permission catalog'),
    ('role.read',            'List and read roles and role assignments'),
    ('role.create',          'Create roles'),
    ('role.update',          'Update roles'),
    ('role.permissions.manage', 'Replace a role''s permissions'),
    ('role.assign',          'Replace a user''s organization role assignments');

COMMENT ON TABLE permissions IS
    'System-wide permission catalog, seeded by migration. Never modified by the client API.';
COMMENT ON TABLE roles IS
    'Organization-scoped role. code is a normalized slug, unique per organization and immutable. status INACTIVE grants nothing while kept in DB.';
COMMENT ON TABLE role_permissions IS
    'Grants of global permissions to an organization role; unique (role_id, permission_code).';
COMMENT ON TABLE user_organization_roles IS
    'User owns a role for the whole organization. Composite FKs enforce the same organization on user and role (no cross-tenant assignment).';