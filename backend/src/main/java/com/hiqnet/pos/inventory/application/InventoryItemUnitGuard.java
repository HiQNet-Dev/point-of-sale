package com.hiqnet.pos.inventory.application;

import com.hiqnet.pos.inventory.domain.InventoryItem;
import com.hiqnet.pos.inventory.domain.InventoryItemRepository;
import com.hiqnet.pos.shared.error.BusinessException;
import com.hiqnet.pos.shared.error.ErrorCodes;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Propagation;
import org.springframework.transaction.annotation.Transactional;

/**
 * Coordina el cambio de unidad base de un {@code InventoryItem} con la creación
 * concurrente de dependencias que fijan el significado de sus cantidades.
 *
 * <p>Antes de persistir cualquier hecho dependiente (componente de receta,
 * efecto de modificador, balance/movimiento de sucursal o partida de orden de
 * compra) el llamante bloquea de forma pesimista la fila del artículo
 * ({@code SELECT ... FOR UPDATE}) en un orden determinista por {@code id} y
 * marca {@code unit_locked = true}. Como el cambio de unidad toma el mismo
 * bloqueo de fila, ambas operaciones se serializan: o la dependencia observa la
 * unidad ya modificada, o el cambio de unidad queda rechazado. Esto elimina la
 * carrera TOCTOU entre comprobar dependencias y actualizar {@code base_unit}.</p>
 *
 * <p>El marcador {@code unit_locked} es persistente porque algunas dependencias
 * pueden borrarse (reemplazo de recetas o de efectos de modificador, eliminación
 * de partidas en borrador) sin que el artículo deje de haber sido utilizado; una
 * comprobación dinámica de filas perdería esa evidencia.</p>
 *
 * <p>Orden global de bloqueos: {@code CashShift → PurchaseOrder → Supplier →
 * InventoryItem → {BranchInventory, ProductInventoryRecipe,
 * ModifierOptionInventoryConfig}}. Los artículos siempre se bloquean por
 * {@code id} ascendente.</p>
 */
@Service
public class InventoryItemUnitGuard {

    private final InventoryItemRepository inventoryItemRepository;

    public InventoryItemUnitGuard(InventoryItemRepository inventoryItemRepository) {
        this.inventoryItemRepository = inventoryItemRepository;
    }

    /**
     * Bloquea los artículos indicados y marca su unidad base como inmutable.
     *
     * <p>Debe invocarse dentro de la transacción que crea la dependencia y antes
     * de leer {@code baseUnit} o de insertar la fila dependiente, de modo que el
     * valor de unidad observado sea el vigente al confirmar.</p>
     *
     * @param organizationId organización propietaria (scope del bloqueo)
     * @param inventoryItemIds artículos referenciados por la dependencia
     * @return artículos bloqueados indexados por id, con su unidad base vigente
     * @throws BusinessException {@code INVENTORY_ITEM_NOT_FOUND} si algún artículo
     *         no existe en la organización
     */
    @Transactional(propagation = Propagation.MANDATORY)
    public Map<UUID, InventoryItem> lockAndMarkReferenced(
            UUID organizationId, Collection<UUID> inventoryItemIds) {
        if (inventoryItemIds == null || inventoryItemIds.isEmpty()) {
            return Map.of();
        }
        List<UUID> ordered = inventoryItemIds.stream().distinct().sorted().toList();
        List<InventoryItem> locked =
                inventoryItemRepository.findByOrganizationIdAndIdInForUpdate(organizationId, ordered);
        if (locked.size() != ordered.size()) {
            throw new BusinessException(
                    HttpStatus.NOT_FOUND,
                    ErrorCodes.INVENTORY_ITEM_NOT_FOUND,
                    "One or more inventory items were not found in the organization");
        }
        inventoryItemRepository.markUnitLocked(organizationId, ordered);
        Map<UUID, InventoryItem> byId = new LinkedHashMap<>();
        locked.forEach(item -> byId.put(item.getId(), item));
        return byId;
    }
}
