//! Handlers preparados: solo adaptación y delegación. No están en el runtime.
use super::{PurchasingState, dto::*};
use crate::dto::IpcError;
use pos_application::purchasing as app;
use pos_core::purchasing::PurchaseOrderStatus;
use tauri::State;

/// purchasing.supplier.write, organización del principal.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_create_supplier(
    state: State<'_, PurchasingState>,
    request: SupplierInput,
) -> Result<SupplierDto, IpcError> {
    let mut c = state.resolve(None).await?;
    Ok(c.application
        .create_supplier(
            &c.principal,
            &mut c.identities,
            request.into(),
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.supplier.read, organización del principal.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_get_supplier(
    state: State<'_, PurchasingState>,
    request: SupplierKey,
) -> Result<SupplierDto, IpcError> {
    let c = state.resolve(None).await?;
    Ok(c.application
        .get_supplier(&c.principal, &id(&request.supplier_id)?)
        .await?
        .into())
}
/// purchasing.supplier.read, página de organización.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_list_suppliers(
    state: State<'_, PurchasingState>,
    request: SupplierList,
) -> Result<Vec<SupplierDto>, IpcError> {
    let c = state.resolve(None).await?;
    Ok(c.application
        .list_suppliers(
            &c.principal,
            request.status.map(SupplierStatus::domain),
            page(request.limit, request.offset),
        )
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}
/// purchasing.supplier.write, CAS de proveedor.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_update_supplier(
    state: State<'_, PurchasingState>,
    request: SupplierUpdate,
) -> Result<SupplierDto, IpcError> {
    let mut c = state.resolve(None).await?;
    let supplier_id = id(&request.supplier_id)?;
    Ok(c.application
        .update_supplier(
            &c.principal,
            &mut c.identities,
            &supplier_id,
            request.application()?,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.order.write, sucursal verificada.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_create_order(
    state: State<'_, PurchasingState>,
    request: OrderInput,
) -> Result<OrderDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    let request = app::NewPurchaseOrderRequest {
        code: request.code,
        supplier_id: id(&request.supplier_id)?,
        lines: request
            .lines
            .into_iter()
            .map(LineInput::application)
            .collect::<Result<_, _>>()?,
    };
    Ok(c.application
        .create_purchase_order(&c.principal, &mut c.identities, request, &c.occurred_at)
        .await?
        .into())
}
/// purchasing.order.read, detalle scoped.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_get_order(
    state: State<'_, PurchasingState>,
    request: OrderKey,
) -> Result<OrderDto, IpcError> {
    let c = state.resolve(Some(&request.branch_id)).await?;
    Ok(c.application
        .get_purchase_order(&c.principal, &id(&request.order_id)?)
        .await?
        .into())
}
/// purchasing.order.read, página por sucursal.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_list_orders(
    state: State<'_, PurchasingState>,
    request: OrderList,
) -> Result<Vec<OrderDto>, IpcError> {
    let c = state.resolve(Some(&request.branch_id)).await?;
    let status = request
        .status
        .as_deref()
        .map(PurchaseOrderStatus::parse)
        .transpose()
        .map_err(|_| IpcError::ValidationError)?;
    Ok(c.application
        .list_purchase_orders(&c.principal, status, page(request.limit, request.offset))
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}
/// purchasing.order.write, proveedor de borrador y CAS.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_change_supplier(
    state: State<'_, PurchasingState>,
    request: SupplierChange,
) -> Result<OrderDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    let change = app::ChangeSupplierRequest {
        expected_version: version(&request.expected_version)?,
        supplier_id: id(&request.supplier_id)?,
    };
    Ok(c.application
        .change_purchase_order_supplier(
            &c.principal,
            &mut c.identities,
            &id(&request.order_id)?,
            change,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.order.write, alta de partida en borrador.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_add_line(
    state: State<'_, PurchasingState>,
    request: LineAdd,
) -> Result<OrderDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    Ok(c.application
        .add_purchase_order_line(
            &c.principal,
            &mut c.identities,
            &id(&request.order_id)?,
            request.line.application()?,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.order.write, edición exacta con CAS y patch explícito de costo.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_update_line(
    state: State<'_, PurchasingState>,
    request: LineUpdate,
) -> Result<OrderDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    let update = app::UpdatePurchaseOrderLineRequest {
        expected_version: version(&request.expected_version)?,
        quantity: request.quantity,
        unit_cost: request.unit_cost.nested(),
    };
    Ok(c.application
        .update_purchase_order_line(
            &c.principal,
            &mut c.identities,
            &id(&request.order_id)?,
            &id(&request.line_id)?,
            update,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.order.write, retirada con CAS.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_remove_line(
    state: State<'_, PurchasingState>,
    request: LineRemove,
) -> Result<OrderDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    Ok(c.application
        .remove_purchase_order_line(
            &c.principal,
            &mut c.identities,
            &id(&request.order_id)?,
            &id(&request.line_id)?,
            version(&request.expected_version)?,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.order.issue, transición irreversible.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_issue_order(
    state: State<'_, PurchasingState>,
    request: OrderTransition,
) -> Result<OrderDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    Ok(c.application
        .issue_purchase_order(
            &c.principal,
            &mut c.identities,
            &id(&request.order_id)?,
            version(&request.expected_version)?,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.order.cancel, transición irreversible.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_cancel_order(
    state: State<'_, PurchasingState>,
    request: OrderTransition,
) -> Result<OrderDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    Ok(c.application
        .cancel_purchase_order(
            &c.principal,
            &mut c.identities,
            &id(&request.order_id)?,
            version(&request.expected_version)?,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.receipt.write, recepción idempotente; valoración económica pendiente.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_post_receipt(
    state: State<'_, PurchasingState>,
    request: ReceiptInput,
) -> Result<ReceiptDto, IpcError> {
    let mut c = state.resolve(Some(&request.branch_id)).await?;
    let lines = request
        .lines
        .into_iter()
        .map(|line| {
            Ok(app::ReceiptLineInput {
                purchase_order_line_id: id(&line.purchase_order_line_id)?,
                quantity: line.quantity,
            })
        })
        .collect::<Result<_, IpcError>>()?;
    let receipt = app::PostReceiptRequest {
        expected_version: version(&request.expected_version)?,
        idempotency_key: request.idempotency_key,
        lines,
    };
    Ok(c.application
        .post_purchase_receipt(
            &c.principal,
            &mut c.identities,
            &id(&request.order_id)?,
            receipt,
            &c.occurred_at,
        )
        .await?
        .into())
}
/// purchasing.receipt.read, detalle scoped.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_get_receipt(
    state: State<'_, PurchasingState>,
    request: ReceiptKey,
) -> Result<ReceiptDto, IpcError> {
    let c = state.resolve(Some(&request.branch_id)).await?;
    Ok(c.application
        .get_purchase_receipt(&c.principal, &id(&request.receipt_id)?)
        .await?
        .into())
}
/// purchasing.receipt.read, página por sucursal y filtro de orden.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_list_receipts(
    state: State<'_, PurchasingState>,
    request: ReceiptList,
) -> Result<Vec<ReceiptDto>, IpcError> {
    let c = state.resolve(Some(&request.branch_id)).await?;
    let order_id = request.order_id.as_deref().map(id).transpose()?;
    Ok(c.application
        .list_purchase_receipts(
            &c.principal,
            order_id.as_ref(),
            page(request.limit, request.offset),
        )
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}
/// purchasing.receipt.read, cantidades acumuladas sin coma flotante.
#[tauri::command]
#[specta::specta]
pub async fn purchasing_receipt_status(
    state: State<'_, PurchasingState>,
    request: OrderKey,
) -> Result<Vec<ReceiptStatusDto>, IpcError> {
    let c = state.resolve(Some(&request.branch_id)).await?;
    Ok(c.application
        .purchase_receipt_status(&c.principal, &id(&request.order_id)?)
        .await?
        .into_iter()
        .map(Into::into)
        .collect())
}
