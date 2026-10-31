//! Consultas SQL del contexto.
//!
//! Solo infraestructura: los casos de uso no conocen estas sentencias.

pub(crate) const SELECT_CATEGORY: &str = "SELECT id, organization_id, code, name, status, version \
     FROM category WHERE organization_id = ? AND id = ?";

pub(crate) const INSERT_CATEGORY: &str = "INSERT INTO category \
     (id, organization_id, code, name, status, version, created_at, updated_at) \
     VALUES (?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const UPDATE_CATEGORY_CAS: &str = "UPDATE category \
     SET name = ?, status = ?, version = ?, updated_at = ? \
     WHERE organization_id = ? AND id = ? AND version = ?";

pub(crate) const LIST_CATEGORIES: &str = "SELECT id, organization_id, code, name, status, version \
     FROM category WHERE organization_id = ? AND (? IS NULL OR status = ?) \
     ORDER BY name, id LIMIT ? OFFSET ?";

pub(crate) const SELECT_PRODUCT: &str = "SELECT id, organization_id, category_id, sku, barcode, name, \
     description, unit_price, status, version \
     FROM product WHERE organization_id = ? AND id = ?";

pub(crate) const INSERT_PRODUCT: &str = "INSERT INTO product \
     (id, organization_id, category_id, sku, barcode, name, description, unit_price, status, \
      version, created_at, updated_at) \
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

pub(crate) const UPDATE_PRODUCT_CAS: &str = "UPDATE product \
     SET category_id = ?, barcode = ?, name = ?, description = ?, unit_price = ?, status = ?, \
         version = ?, updated_at = ? \
     WHERE organization_id = ? AND id = ? AND version = ?";

pub(crate) const LIST_PRODUCTS: &str = "SELECT id, organization_id, category_id, sku, barcode, name, \
     description, unit_price, status, version \
     FROM product WHERE organization_id = ? AND (? IS NULL OR status = ?) \
       AND (? IS NULL OR category_id = ?) \
     ORDER BY name, id LIMIT ? OFFSET ?";
