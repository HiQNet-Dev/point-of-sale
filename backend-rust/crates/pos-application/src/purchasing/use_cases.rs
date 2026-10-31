//! Casos de uso de compras sobre puertos.
//!
//! Reproduce las 17 operaciones de `purchasing` de Java: proveedores, órdenes de
//! compra y recepciones. No hay reglas nuevas: se reutilizan los agregados de
//! `pos-core` y se añade autorización contextual, CAS y auditoría transaccional.
//!
//! # Orden de una mutación
//!
//! ```text
//! principal verificado → permiso y scope → cargar estado → regla de dominio
//!   → mutar → actualizar/CAS → auditoría → COMMIT
//! ```

use pos_core::{
    audit::AuditOccurredAt,
    catalog::{CatalogCode, UnitPrice},
    identity::VerifiedPrincipal,
    inventory::Quantity,
    organization_branch::{ApplicationError, EntityId, IdentityGenerator, Permission, Status},
    purchasing::{
        NewPurchaseOrder, NewPurchaseOrderLine, NewPurchaseReceipt, PurchaseOrder,
        PurchaseOrderLine, PurchaseOrderStatus, PurchaseReceipt, PurchaseReceiptLine,
        ReceiptLineRequest, validate_receipt_lines,
    },
    supplier::SupplierError,
};
use serde_json::json;

use crate::{
    CasOutcome,
    audit::append_audit,
    authorization::{require_branch_scope, require_organization_scope},
    purchasing::{dto::*, ports::*},
};

/// Tamaño máximo de página del contrato de compras.
pub const MAX_PURCHASING_PAGE_SIZE: i64 = 100;

/// Casos de uso de compras.
#[derive(Clone)]
pub struct PurchasingApplication<U> {
    unit_of_work: U,
}

impl<U> PurchasingApplication<U>
where
    U: PurchasingUnitOfWork,
    U::Transaction: Send,
{
    /// Ensambla los casos de uso sobre una unidad de trabajo ya configurada.
    pub fn new(unit_of_work: U) -> Self {
        Self { unit_of_work }
    }

    // ---------------------------------------------------------------------
    // Proveedores
    // ---------------------------------------------------------------------

    /// Crea un proveedor activo de la organización del principal.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] sin permiso,
    /// [`ApplicationError::SupplierCodeAlreadyExists`] si el slug está ocupado y
    /// [`ApplicationError::InvalidPurchaseInput`] si algún campo no cumple.
    pub async fn create_supplier<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewSupplierRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<SupplierSummary, ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::PurchasingSupplierWrite)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let supplier = pos_core::supplier::Supplier::create(
            request
                .into_new_supplier(identities.next_id()?, organization_id.clone())
                .map_err(map_supplier_error)?,
        );
        if transaction
            .supplier_code_exists(&organization_id, supplier.code())
            .await?
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::SupplierCodeAlreadyExists);
        }
        transaction
            .insert_supplier(&supplier, occurred_at.as_str())
            .await?;
        append_audit(
            &mut transaction,
            identities,
            principal,
            "supplier.created",
            "supplier",
            supplier.id().clone(),
            json!({ "code": supplier.code().as_str() }),
            occurred_at,
        )
        .await?;
        let summary = SupplierSummary::from(&supplier);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(summary)
    }

    /// Consulta un proveedor de la organización.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] sin permiso y
    /// [`ApplicationError::SupplierNotFound`] si no existe en el scope.
    pub async fn get_supplier(
        &self,
        principal: &VerifiedPrincipal,
        supplier_id: &EntityId,
    ) -> Result<SupplierSummary, ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::PurchasingSupplierRead)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let supplier = transaction
            .get_supplier(&organization_id, supplier_id)
            .await?;
        let _ = self.unit_of_work.rollback_transaction(transaction).await;
        supplier
            .map(|supplier| SupplierSummary::from(&supplier))
            .ok_or(ApplicationError::SupplierNotFound)
    }

    /// Lista proveedores de la organización.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] sin permiso y
    /// [`ApplicationError::InvalidPurchaseInput`] con página fuera de contrato.
    pub async fn list_suppliers(
        &self,
        principal: &VerifiedPrincipal,
        status: Option<Status>,
        page: PurchasingPage,
    ) -> Result<Vec<SupplierSummary>, ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::PurchasingSupplierRead)?;
        if !page.is_valid() {
            return Err(ApplicationError::InvalidPurchaseInput);
        }
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let suppliers = transaction
            .list_suppliers(&organization_id, status, page.limit, page.offset)
            .await?;
        let _ = self.unit_of_work.rollback_transaction(transaction).await;
        Ok(suppliers.iter().map(SupplierSummary::from).collect())
    }

    /// Actualiza un proveedor con CAS.
    ///
    /// # Errors
    /// [`ApplicationError::SupplierNotFound`], [`ApplicationError::Forbidden`],
    /// [`ApplicationError::ConcurrentModification`] o entrada inválida.
    pub async fn update_supplier<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        supplier_id: &EntityId,
        request: UpdateSupplierRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<SupplierSummary, ApplicationError> {
        let organization_id =
            require_organization_scope(principal, Permission::PurchasingSupplierWrite)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let mut supplier = transaction
            .get_supplier(&organization_id, supplier_id)
            .await?
            .ok_or(ApplicationError::SupplierNotFound)?;
        let previous = supplier.revision();
        let patch = request.into_patch().map_err(map_supplier_error)?;
        let changed = supplier.patch(patch).map_err(map_supplier_error)?;
        if !changed {
            let summary = SupplierSummary::from(&supplier);
            self.unit_of_work.commit_transaction(transaction).await?;
            return Ok(summary);
        }
        match transaction
            .update_supplier_cas(previous, &supplier, occurred_at.as_str())
            .await?
        {
            CasOutcome::Updated => {}
            CasOutcome::Missing => {
                let _ = self.unit_of_work.rollback_transaction(transaction).await;
                return Err(ApplicationError::SupplierNotFound);
            }
            CasOutcome::Conflict => {
                let _ = self.unit_of_work.rollback_transaction(transaction).await;
                return Err(ApplicationError::ConcurrentModification);
            }
        }
        append_audit(
            &mut transaction,
            identities,
            principal,
            "supplier.updated",
            "supplier",
            supplier.id().clone(),
            json!({ "version": supplier.revision() }),
            occurred_at,
        )
        .await?;
        let summary = SupplierSummary::from(&supplier);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(summary)
    }

    // ---------------------------------------------------------------------
    // Órdenes de compra
    // ---------------------------------------------------------------------

    /// Crea una orden `DRAFT` con partidas opcionales.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`], [`ApplicationError::SupplierNotFound`],
    /// [`ApplicationError::SupplierInactive`], [`ApplicationError::InvalidPurchaseInput`]
    /// o [`ApplicationError::PurchaseOrderCodeAlreadyExists`].
    pub async fn create_purchase_order<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        request: NewPurchaseOrderRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderWrite, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let currency = transaction
            .organization_currency(&organization_id)
            .await?
            .ok_or(ApplicationError::OrganizationNotFound)?;
        let supplier = transaction
            .get_supplier(&organization_id, &request.supplier_id)
            .await?
            .ok_or(ApplicationError::SupplierNotFound)?;
        if !supplier.is_active() {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::SupplierInactive);
        }
        let code =
            CatalogCode::new(&request.code).map_err(|_| ApplicationError::InvalidPurchaseInput)?;
        if transaction
            .purchase_order_code_exists(&organization_id, &code)
            .await?
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::PurchaseOrderCodeAlreadyExists);
        }
        let snapshot = supplier.snapshot();
        let order = PurchaseOrder::create(NewPurchaseOrder {
            id: identities.next_id()?,
            organization_id: organization_id.clone(),
            branch_id: branch_id.clone(),
            supplier_id: supplier.id().clone(),
            code,
            currency,
            supplier_code_snapshot: snapshot.code,
            supplier_name_snapshot: snapshot.name,
            created_by_user_id: principal.user_id().clone(),
        });
        transaction
            .insert_purchase_order(&order, occurred_at.as_str())
            .await?;
        let mut lines = Vec::with_capacity(request.lines.len());
        for (index, line) in request.lines.iter().enumerate() {
            let number =
                u32::try_from(index + 1).map_err(|_| ApplicationError::InvalidPurchaseInput)?;
            let id = identities.next_id()?;
            let built = self
                .build_line(
                    &mut transaction,
                    id,
                    &organization_id,
                    order.id(),
                    number,
                    line,
                )
                .await?;
            transaction
                .insert_purchase_order_line(&built, occurred_at.as_str())
                .await?;
            lines.push(built);
        }
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.order.created",
            "purchase_order",
            order.id().clone(),
            json!({ "code": order.code().as_str(), "lines": lines.len() }),
            occurred_at,
        )
        .await?;
        let view = order_view(&order, &lines, &[]);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    /// Consulta una orden con sus partidas en la sucursal del principal.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] o [`ApplicationError::PurchaseOrderNotFound`].
    pub async fn get_purchase_order(
        &self,
        principal: &VerifiedPrincipal,
        purchase_order_id: &EntityId,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderRead, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (order, lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let _ = self.unit_of_work.rollback_transaction(transaction).await;
        Ok(order_view(&order, &lines, &received))
    }

    /// Lista órdenes de la sucursal con filtro de estado.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] o página inválida.
    pub async fn list_purchase_orders(
        &self,
        principal: &VerifiedPrincipal,
        status: Option<PurchaseOrderStatus>,
        page: PurchasingPage,
    ) -> Result<Vec<PurchaseOrderView>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderRead, false)?;
        if !page.is_valid() {
            return Err(ApplicationError::InvalidPurchaseInput);
        }
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let orders = transaction
            .list_purchase_orders(
                &organization_id,
                &branch_id,
                status,
                page.limit,
                page.offset,
            )
            .await?;
        let mut views = Vec::with_capacity(orders.len());
        for (order, lines) in orders {
            let received = transaction
                .received_quantities(&organization_id, order.id())
                .await?;
            views.push(order_view(&order, &lines, &received));
        }
        let _ = self.unit_of_work.rollback_transaction(transaction).await;
        Ok(views)
    }

    /// Cambia el proveedor de un borrador con CAS.
    ///
    /// # Errors
    /// [`ApplicationError::PurchaseOrderNotFound`], [`ApplicationError::SupplierNotFound`],
    /// [`ApplicationError::PurchaseOrderNotDraft`] o [`ApplicationError::ConcurrentModification`].
    pub async fn change_purchase_order_supplier<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        purchase_order_id: &EntityId,
        request: ChangeSupplierRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderWrite, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (mut order, lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        let supplier = transaction
            .get_supplier(&organization_id, &request.supplier_id)
            .await?
            .ok_or(ApplicationError::SupplierNotFound)?;
        if !supplier.is_active() {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::SupplierInactive);
        }
        let snapshot = supplier.snapshot();
        let previous = order.revision();
        let changed = order
            .change_supplier(
                request.expected_version,
                supplier.id().clone(),
                snapshot.code,
                snapshot.name,
            )
            .map_err(map_purchasing_error)?;
        if !changed {
            let received = transaction
                .received_quantities(&organization_id, order.id())
                .await?;
            let view = order_view(&order, &lines, &received);
            self.unit_of_work.commit_transaction(transaction).await?;
            return Ok(view);
        }
        if transaction
            .update_purchase_order_cas(previous, &order, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.order.supplier_changed",
            "purchase_order",
            order.id().clone(),
            json!({ "supplierId": supplier.id().as_str() }),
            occurred_at,
        )
        .await?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let view = order_view(&order, &lines, &received);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    /// Añade una partida a un borrador.
    ///
    /// # Errors
    /// [`ApplicationError::PurchaseOrderNotFound`], [`ApplicationError::PurchaseOrderNotDraft`],
    /// [`ApplicationError::InventoryItemNotFound`] o entrada inválida.
    pub async fn add_purchase_order_line<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        purchase_order_id: &EntityId,
        request: NewPurchaseOrderLineRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderWrite, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (mut order, mut lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        let number = transaction
            .next_line_number(&organization_id, order.id())
            .await?;
        let id = identities.next_id()?;
        let line = self
            .build_line(
                &mut transaction,
                id,
                &organization_id,
                order.id(),
                number,
                &request,
            )
            .await?;
        let previous = order.revision();
        order
            .register_line_mutation()
            .map_err(map_purchasing_error)?;
        transaction
            .insert_purchase_order_line(&line, occurred_at.as_str())
            .await?;
        if transaction
            .update_purchase_order_cas(previous, &order, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        lines.push(line);
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.order.line_added",
            "purchase_order",
            order.id().clone(),
            json!({ "lineNumber": number }),
            occurred_at,
        )
        .await?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let view = order_view(&order, &lines, &received);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    /// Edita cantidad y costo de una partida de un borrador.
    ///
    /// # Errors
    /// [`ApplicationError::PurchaseOrderNotFound`], [`ApplicationError::PurchaseOrderLineNotFound`],
    /// [`ApplicationError::PurchaseOrderNotDraft`] o entrada inválida.
    pub async fn update_purchase_order_line<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        purchase_order_id: &EntityId,
        line_id: &EntityId,
        request: UpdatePurchaseOrderLineRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderWrite, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (mut order, mut lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        if order.revision() != request.expected_version {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        order.require_draft().map_err(map_purchasing_error)?;
        let position = lines
            .iter()
            .position(|line| line.id() == line_id)
            .ok_or(ApplicationError::PurchaseOrderLineNotFound)?;
        let current = lines[position].clone();
        let quantity = parse_positive_quantity(&request.quantity)
            .ok_or(ApplicationError::InvalidPurchaseInput)?;
        let unit_cost = match request.unit_cost {
            None => current.unit_cost(),
            Some(value) => parse_optional_cost(value.as_deref())
                .ok_or(ApplicationError::InvalidPurchaseInput)?,
        };
        let updated = PurchaseOrderLine::restore(
            current.id().clone(),
            current.organization_id().clone(),
            current.purchase_order_id().clone(),
            current.inventory_item_id().clone(),
            current.line_number(),
            current.item_code_snapshot().clone(),
            current.item_name_snapshot().clone(),
            current.unit_snapshot(),
            quantity,
            unit_cost,
        )
        .map_err(map_purchasing_error)?;
        let previous = order.revision();
        order
            .register_line_mutation()
            .map_err(map_purchasing_error)?;
        transaction
            .update_purchase_order_line(&updated, occurred_at.as_str())
            .await?;
        if transaction
            .update_purchase_order_cas(previous, &order, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        lines[position] = updated;
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.order.line_updated",
            "purchase_order",
            order.id().clone(),
            json!({ "lineId": line_id.as_str() }),
            occurred_at,
        )
        .await?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let view = order_view(&order, &lines, &received);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    /// Retira una partida de un borrador.
    ///
    /// # Errors
    /// [`ApplicationError::PurchaseOrderNotFound`], [`ApplicationError::PurchaseOrderLineNotFound`],
    /// [`ApplicationError::PurchaseOrderNotDraft`] o [`ApplicationError::ConcurrentModification`].
    pub async fn remove_purchase_order_line<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        purchase_order_id: &EntityId,
        line_id: &EntityId,
        expected_version: u64,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderWrite, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (mut order, lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        if !lines.iter().any(|line| line.id() == line_id) {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::PurchaseOrderLineNotFound);
        }
        if order.revision() != expected_version {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        let previous = order.revision();
        order
            .register_line_mutation()
            .map_err(map_purchasing_error)?;
        transaction
            .delete_purchase_order_line(&organization_id, order.id(), line_id)
            .await?;
        if transaction
            .update_purchase_order_cas(previous, &order, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        let remaining: Vec<PurchaseOrderLine> = lines
            .iter()
            .filter(|line| line.id() != line_id)
            .cloned()
            .collect();
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.order.line_removed",
            "purchase_order",
            order.id().clone(),
            json!({ "lineId": line_id.as_str() }),
            occurred_at,
        )
        .await?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let view = order_view(&order, &remaining, &received);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    /// Emite una orden: transición irreversible `DRAFT → ORDERED`.
    ///
    /// # Errors
    /// [`ApplicationError::PurchaseOrderNotFound`], [`ApplicationError::PurchaseOrderNotDraft`]
    /// o [`ApplicationError::ConcurrentModification`].
    pub async fn issue_purchase_order<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        purchase_order_id: &EntityId,
        expected_version: u64,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderIssue, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (mut order, lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        if order.revision() != expected_version {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        let previous = order.revision();
        let at = transaction.utc_seconds(occurred_at).await?;
        order
            .issue(principal.user_id().clone(), at)
            .map_err(map_purchasing_error)?;
        if transaction
            .update_purchase_order_cas(previous, &order, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.order.issued",
            "purchase_order",
            order.id().clone(),
            json!({ "version": order.revision() }),
            occurred_at,
        )
        .await?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let view = order_view(&order, &lines, &received);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    /// Cancela una orden desde `DRAFT` u `ORDERED`.
    ///
    /// # Errors
    /// [`ApplicationError::PurchaseOrderNotFound`], [`ApplicationError::PurchaseOrderAlreadyCancelled`]
    /// o [`ApplicationError::ConcurrentModification`].
    pub async fn cancel_purchase_order<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        purchase_order_id: &EntityId,
        expected_version: u64,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseOrderView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingOrderCancel, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (mut order, lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        if order.revision() != expected_version {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        let previous = order.revision();
        let at = transaction.utc_seconds(occurred_at).await?;
        order
            .cancel(principal.user_id().clone(), at)
            .map_err(map_purchasing_error)?;
        if transaction
            .update_purchase_order_cas(previous, &order, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.order.cancelled",
            "purchase_order",
            order.id().clone(),
            json!({ "version": order.revision() }),
            occurred_at,
        )
        .await?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let view = order_view(&order, &lines, &received);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    // ---------------------------------------------------------------------
    // Recepciones
    // ---------------------------------------------------------------------

    /// Registra una recepción: stock, ledger, estado y auditoría en una UoW.
    ///
    /// Repetir la misma clave de idempotencia devuelve la recepción existente sin
    /// mover stock otra vez.
    ///
    /// # Errors
    /// [`ApplicationError::PurchaseOrderNotFound`], [`ApplicationError::PurchaseOrderNotReceivable`],
    /// [`ApplicationError::OverReceipt`], [`ApplicationError::ConcurrentModification`]
    /// o entrada inválida.
    pub async fn post_purchase_receipt<I: IdentityGenerator>(
        &self,
        principal: &VerifiedPrincipal,
        identities: &mut I,
        purchase_order_id: &EntityId,
        request: PostReceiptRequest,
        occurred_at: &AuditOccurredAt,
    ) -> Result<PurchaseReceiptView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingReceiptWrite, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;

        // Idempotencia: la misma clave devuelve el hecho ya confirmado.
        if let Some(existing) = transaction
            .find_receipt_by_idempotency(&organization_id, &request.idempotency_key)
            .await?
        {
            let lines = transaction
                .receipt_lines(&organization_id, existing.id())
                .await?;
            let view = receipt_view(&existing, &lines);
            self.unit_of_work.commit_transaction(transaction).await?;
            return Ok(view);
        }

        let (mut order, order_lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        order.ensure_receivable().map_err(map_purchasing_error)?;
        if order.revision() != request.expected_version {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }

        let mut requested = Vec::with_capacity(request.lines.len());
        for line in &request.lines {
            let quantity = parse_positive_quantity(&line.quantity)
                .ok_or(ApplicationError::InvalidPurchaseInput)?;
            requested.push(ReceiptLineRequest {
                purchase_order_line_id: line.purchase_order_line_id.clone(),
                quantity,
            });
        }
        validate_receipt_lines(&requested).map_err(map_purchasing_error)?;

        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let mut new_totals: Vec<(EntityId, Quantity)> = Vec::with_capacity(requested.len());
        for line in &requested {
            let order_line = order_lines
                .iter()
                .find(|candidate| candidate.id() == &line.purchase_order_line_id)
                .ok_or(ApplicationError::PurchaseOrderLineNotFound)?;
            let already = received
                .iter()
                .find(|(id, _)| id == order_line.id())
                .map_or_else(Quantity::zero, |(_, quantity)| *quantity);
            let total = already
                .checked_add(&line.quantity)
                .map_err(|_| ApplicationError::InvalidPurchaseInput)?;
            if total.as_minor() > order_line.quantity().as_minor() {
                let _ = self.unit_of_work.rollback_transaction(transaction).await;
                return Err(ApplicationError::OverReceipt);
            }
            new_totals.push((order_line.id().clone(), total));
        }

        let received_at = transaction.utc_seconds(occurred_at).await?;
        let receipt = PurchaseReceipt::create(
            NewPurchaseReceipt {
                id: identities.next_id()?,
                organization_id: organization_id.clone(),
                branch_id: branch_id.clone(),
                purchase_order_id: order.id().clone(),
                idempotency_key: request.idempotency_key.clone(),
                actor_user_id: principal.user_id().clone(),
            },
            received_at.get(),
        )?;
        transaction
            .insert_purchase_receipt(&receipt, occurred_at.as_str())
            .await?;

        let mut stored_lines = Vec::with_capacity(requested.len());
        for line in &requested {
            let order_line = order_lines
                .iter()
                .find(|candidate| candidate.id() == &line.purchase_order_line_id)
                .ok_or(ApplicationError::PurchaseOrderLineNotFound)?;
            let receipt_line = PurchaseReceiptLine::create(
                identities.next_id()?,
                organization_id.clone(),
                receipt.id().clone(),
                order_line.id().clone(),
                order_line.inventory_item_id().clone(),
                line.quantity,
            )?;
            transaction
                .insert_purchase_receipt_line(&receipt_line, occurred_at.as_str())
                .await?;
            let ledger_id = identities.next_id()?;
            let balance_id = identities.next_id()?;
            transaction
                .apply_purchase_receipt_stock(
                    &organization_id,
                    &branch_id,
                    order_line.inventory_item_id(),
                    line.quantity,
                    receipt_line.id(),
                    &ledger_id,
                    &balance_id,
                    principal.user_id(),
                    received_at.get(),
                    occurred_at.as_str(),
                )
                .await?;
            stored_lines.push(receipt_line);
        }

        // Estado siguiente: RECEIVED solo si todas las partidas quedan completas.
        let mut fully_received = true;
        for order_line in &order_lines {
            let total = new_totals
                .iter()
                .find(|(id, _)| id == order_line.id())
                .map(|(_, quantity)| *quantity)
                .or_else(|| {
                    received
                        .iter()
                        .find(|(id, _)| id == order_line.id())
                        .map(|(_, quantity)| *quantity)
                })
                .unwrap_or_else(Quantity::zero);
            if total.as_minor() < order_line.quantity().as_minor() {
                fully_received = false;
                break;
            }
        }
        let next = if fully_received {
            PurchaseOrderStatus::Received
        } else {
            PurchaseOrderStatus::PartiallyReceived
        };
        let previous = order.revision();
        order.apply_receipt(next).map_err(map_purchasing_error)?;
        if transaction
            .update_purchase_order_cas(previous, &order, occurred_at.as_str())
            .await?
            != CasOutcome::Updated
        {
            let _ = self.unit_of_work.rollback_transaction(transaction).await;
            return Err(ApplicationError::ConcurrentModification);
        }
        append_audit(
            &mut transaction,
            identities,
            principal,
            "purchase.receipt.posted",
            "purchase_receipt",
            receipt.id().clone(),
            json!({
                "purchaseOrderId": order.id().as_str(),
                "lines": stored_lines.len(),
                "status": order.status().as_str(),
            }),
            occurred_at,
        )
        .await?;
        let view = receipt_view(&receipt, &stored_lines);
        self.unit_of_work.commit_transaction(transaction).await?;
        Ok(view)
    }

    /// Consulta una recepción con sus líneas.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] o [`ApplicationError::PurchaseReceiptNotFound`].
    pub async fn get_purchase_receipt(
        &self,
        principal: &VerifiedPrincipal,
        purchase_receipt_id: &EntityId,
    ) -> Result<PurchaseReceiptView, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingReceiptRead, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let receipt = transaction
            .get_purchase_receipt(&organization_id, &branch_id, purchase_receipt_id)
            .await?
            .ok_or(ApplicationError::PurchaseReceiptNotFound)?;
        let lines = transaction
            .receipt_lines(&organization_id, receipt.id())
            .await?;
        let _ = self.unit_of_work.rollback_transaction(transaction).await;
        Ok(receipt_view(&receipt, &lines))
    }

    /// Lista recepciones de la sucursal.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] o página inválida.
    pub async fn list_purchase_receipts(
        &self,
        principal: &VerifiedPrincipal,
        purchase_order_id: Option<&EntityId>,
        page: PurchasingPage,
    ) -> Result<Vec<PurchaseReceiptView>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingReceiptRead, false)?;
        if !page.is_valid() {
            return Err(ApplicationError::InvalidPurchaseInput);
        }
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let receipts = transaction
            .list_purchase_receipts(
                &organization_id,
                &branch_id,
                purchase_order_id,
                page.limit,
                page.offset,
            )
            .await?;
        let mut views = Vec::with_capacity(receipts.len());
        for receipt in receipts {
            let lines = transaction
                .receipt_lines(&organization_id, receipt.id())
                .await?;
            views.push(receipt_view(&receipt, &lines));
        }
        let _ = self.unit_of_work.rollback_transaction(transaction).await;
        Ok(views)
    }

    /// Estado acumulado de recepción por partida de una orden.
    ///
    /// # Errors
    /// [`ApplicationError::Forbidden`] o [`ApplicationError::PurchaseOrderNotFound`].
    pub async fn purchase_receipt_status(
        &self,
        principal: &VerifiedPrincipal,
        purchase_order_id: &EntityId,
    ) -> Result<Vec<ReceiptStatusView>, ApplicationError> {
        let (organization_id, branch_id) =
            require_branch_scope(principal, Permission::PurchasingReceiptRead, false)?;
        let mut transaction = self.unit_of_work.begin_transaction().await?;
        let (order, lines) = transaction
            .get_purchase_order(&organization_id, &branch_id, purchase_order_id)
            .await?
            .ok_or(ApplicationError::PurchaseOrderNotFound)?;
        let received = transaction
            .received_quantities(&organization_id, order.id())
            .await?;
        let _ = self.unit_of_work.rollback_transaction(transaction).await;
        Ok(receipt_status(&lines, &received))
    }

    /// Construye una partida validando cantidad, costo y artículo.
    async fn build_line(
        &self,
        transaction: &mut U::Transaction,
        id: EntityId,
        organization_id: &EntityId,
        purchase_order_id: &EntityId,
        line_number: u32,
        request: &NewPurchaseOrderLineRequest,
    ) -> Result<PurchaseOrderLine, ApplicationError> {
        let item = transaction
            .get_inventory_item(organization_id, &request.inventory_item_id)
            .await?
            .ok_or(ApplicationError::InventoryItemNotFound)?;
        if !item.is_active() {
            return Err(ApplicationError::InventoryItemInactive);
        }
        let quantity = parse_positive_quantity(&request.quantity)
            .ok_or(ApplicationError::InvalidPurchaseInput)?;
        item.ensure_quantity_fits_unit(quantity)
            .map_err(|_| ApplicationError::InvalidPurchaseInput)?;
        let unit_cost = parse_optional_cost(request.unit_cost.as_deref())
            .ok_or(ApplicationError::InvalidPurchaseInput)?;
        PurchaseOrderLine::create(NewPurchaseOrderLine {
            id,
            organization_id: organization_id.clone(),
            purchase_order_id: purchase_order_id.clone(),
            inventory_item_id: item.id().clone(),
            line_number,
            item_code_snapshot: item.code().clone(),
            item_name_snapshot: item.name().clone(),
            unit_snapshot: item.base_unit(),
            quantity,
            unit_cost,
        })
        .map_err(map_purchasing_error)
    }
}

fn map_supplier_error(error: SupplierError) -> ApplicationError {
    match error {
        SupplierError::ConcurrentModification => ApplicationError::ConcurrentModification,
        SupplierError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        SupplierError::InvalidPersistedValue | SupplierError::RevisionOverflow => {
            ApplicationError::Storage
        }
        SupplierError::TextTooLong | SupplierError::InvalidEmail => {
            ApplicationError::InvalidPurchaseInput
        }
    }
}

fn map_purchasing_error(error: pos_core::purchasing::PurchasingError) -> ApplicationError {
    use pos_core::purchasing::PurchasingError;
    match error {
        PurchasingError::InvalidQuantity
        | PurchasingError::InvalidLineNumber
        | PurchasingError::InvalidReceiptStatus
        | PurchasingError::DuplicateReceiptLine
        | PurchasingError::UnitChanged => ApplicationError::InvalidPurchaseInput,
        PurchasingError::OrderNotDraft => ApplicationError::PurchaseOrderNotDraft,
        PurchasingError::OrderAlreadyCancelled => ApplicationError::PurchaseOrderAlreadyCancelled,
        PurchasingError::OrderNotIssued
        | PurchasingError::OrderAlreadyReceived
        | PurchasingError::OrderNotReceivable => ApplicationError::PurchaseOrderNotReceivable,
        PurchasingError::ReceiptExceedsOrdered => ApplicationError::OverReceipt,
        PurchasingError::ConcurrentModification => ApplicationError::ConcurrentModification,
        PurchasingError::OwnershipMismatch => ApplicationError::OwnershipMismatch,
        PurchasingError::RevisionOverflow | PurchasingError::InvalidPersistedValue => {
            ApplicationError::Storage
        }
    }
}

fn order_view(
    order: &PurchaseOrder,
    lines: &[PurchaseOrderLine],
    received: &[(EntityId, Quantity)],
) -> PurchaseOrderView {
    let mut total_minor: Option<i128> = Some(0);
    let mut views = Vec::with_capacity(lines.len());
    for line in lines {
        let received_quantity = received
            .iter()
            .find(|(id, _)| id == line.id())
            .map_or_else(Quantity::zero, |(_, quantity)| *quantity);
        let line_total = line
            .unit_cost()
            .and_then(|cost| multiply_cost(cost, line.quantity()));
        total_minor = match (total_minor, line_total.as_deref()) {
            (Some(accumulated), Some(value)) => {
                parse_fixed(value, 10).and_then(|minor| accumulated.checked_add(minor))
            }
            _ => None,
        };
        views.push(PurchaseOrderLineView {
            id: line.id().as_str().to_owned(),
            line_number: line.line_number(),
            inventory_item_id: line.inventory_item_id().as_str().to_owned(),
            item_code: line.item_code_snapshot().as_str().to_owned(),
            item_name: line.item_name_snapshot().as_str().to_owned(),
            unit: line.unit_snapshot().as_str().to_owned(),
            quantity: line.quantity().to_string(),
            unit_cost: line.unit_cost().map(|cost| cost.to_string()),
            line_total,
            received_quantity: received_quantity.to_string(),
            fully_received: received_quantity.as_minor() >= line.quantity().as_minor(),
        });
    }
    PurchaseOrderView {
        id: order.id().as_str().to_owned(),
        branch_id: order.branch_id().as_str().to_owned(),
        supplier_id: order.supplier_id().as_str().to_owned(),
        code: order.code().as_str().to_owned(),
        status: order.status().as_str().to_owned(),
        currency: order.currency().as_str().to_owned(),
        supplier_code: order.supplier_code_snapshot().as_str().to_owned(),
        supplier_name: order.supplier_name_snapshot().as_str().to_owned(),
        version: order.revision().to_string(),
        total: total_minor.map(format_minor_10),
        lines: views,
    }
}

fn receipt_view(receipt: &PurchaseReceipt, lines: &[PurchaseReceiptLine]) -> PurchaseReceiptView {
    PurchaseReceiptView {
        id: receipt.id().as_str().to_owned(),
        purchase_order_id: receipt.purchase_order_id().as_str().to_owned(),
        idempotency_key: receipt.idempotency_key().to_owned(),
        received_at: receipt.received_at_seconds().to_string(),
        lines: lines.iter().map(PurchaseReceiptLineView::from).collect(),
    }
}

fn receipt_status(
    lines: &[PurchaseOrderLine],
    received: &[(EntityId, Quantity)],
) -> Vec<ReceiptStatusView> {
    lines
        .iter()
        .map(|line| {
            let received_quantity = received
                .iter()
                .find(|(id, _)| id == line.id())
                .map_or_else(Quantity::zero, |(_, quantity)| *quantity);
            let pending = line
                .quantity()
                .as_minor()
                .saturating_sub(received_quantity.as_minor());
            ReceiptStatusView {
                purchase_order_line_id: line.id().as_str().to_owned(),
                ordered_quantity: line.quantity().to_string(),
                received_quantity: received_quantity.to_string(),
                pending_quantity: Quantity::from_minor(pending)
                    .map_or_else(|_| "0.000000".to_owned(), |value| value.to_string()),
                fully_received: received_quantity.as_minor() >= line.quantity().as_minor(),
            }
        })
        .collect()
}

/// Total de una partida en escala 10, o `None` si el costo es desconocido.
///
/// El costo pactado usa escala 4 y la cantidad escala 6; su producto cabe
/// exactamente en escala 10, así que no hay redondeo ni pérdida de precisión.
fn multiply_cost(cost: UnitPrice, quantity: Quantity) -> Option<String> {
    let cost_minor = parse_fixed(&cost.to_string(), 4)?;
    let product = cost_minor.checked_mul(quantity.as_minor())?;
    Some(format_minor_10(product))
}

/// Convierte un decimal exacto con la escala indicada a su entero de menor unidad.
fn parse_fixed(value: &str, scale: usize) -> Option<i128> {
    let (integer, fraction) = value.split_once('.')?;
    let mut padded = fraction.to_owned();
    while padded.len() < scale {
        padded.push('0');
    }
    let fraction_minor: i128 = padded.get(..scale)?.parse().ok()?;
    let integer: i128 = integer.parse().ok()?;
    let factor = 10i128.checked_pow(u32::try_from(scale).ok()?)?;
    integer.checked_mul(factor)?.checked_add(fraction_minor)
}

/// Formatea un entero de escala 10 como decimal de 10 decimales.
fn format_minor_10(value: i128) -> String {
    let negative = value < 0;
    let magnitude = value.unsigned_abs();
    let integer = magnitude / 10_000_000_000;
    let fraction = magnitude % 10_000_000_000;
    let sign = if negative { "-" } else { "" };
    format!("{sign}{integer}.{fraction:010}")
}

/// Total derivado de la orden; `None` si alguna partida desconoce su costo.
pub fn supplier_total(view: &PurchaseOrderView) -> Option<&str> {
    view.total.as_deref()
}
