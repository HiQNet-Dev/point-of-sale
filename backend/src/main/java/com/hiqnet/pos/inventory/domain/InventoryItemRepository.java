package com.hiqnet.pos.inventory.domain;

import jakarta.persistence.LockModeType;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.JpaSpecificationExecutor;
import org.springframework.data.jpa.repository.Lock;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;

public interface InventoryItemRepository
        extends JpaRepository<InventoryItem, UUID>, JpaSpecificationExecutor<InventoryItem> {

    Optional<InventoryItem> findByIdAndOrganizationId(UUID id, UUID organizationId);

    List<InventoryItem> findByOrganizationIdAndIdIn(UUID organizationId, Collection<UUID> ids);

    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select item from InventoryItem item where item.id = :id and item.organization.id = :organizationId")
    Optional<InventoryItem> findByIdAndOrganizationIdForUpdate(
            @Param("id") UUID id, @Param("organizationId") UUID organizationId);

    Page<InventoryItem> findByOrganizationId(UUID organizationId, Pageable pageable);

    boolean existsByOrganizationIdAndCode(UUID organizationId, String code);

    /**
     * Bloquea de forma pesimista ({@code SELECT ... FOR UPDATE}) los artículos
     * indicados en un orden determinista por {@code id}, para serializar la
     * creación de dependencias con el cambio de unidad base.
     *
     * @param organizationId organización propietaria (scope del bloqueo)
     * @param ids identificadores de los artículos referenciados
     * @return artículos bloqueados ordenados por {@code id}
     */
    @Lock(LockModeType.PESSIMISTIC_WRITE)
    @Query("select i from InventoryItem i where i.organization.id = :organizationId and i.id in :ids order by i.id asc")
    List<InventoryItem> findByOrganizationIdAndIdInForUpdate(
            @Param("organizationId") UUID organizationId, @Param("ids") Collection<UUID> ids);

    /**
     * Bloquea ({@code FOR NO KEY UPDATE}) los artículos indicados en orden
     * determinista por {@code id} y devuelve su estado y unidad base vigentes como
     * proyección escalar.
     *
     * <p>Es una lectura bloqueada nativa que no usa el Persistence Context: la
     * consulta de entidad equivalente podría devolver una instancia ya administrada
     * (por ejemplo al emitir una orden, que ya referenció el artículo) sin reflejar
     * una desactivación o un cambio de unidad base ya confirmados por otra
     * transacción. La proyección escalar bajo el bloqueo lee el valor vigente al
     * confirmar y cierra esa ventana TOCTOU. El orden ascendente por {@code id}
     * mantiene el orden global de bloqueos y evita interbloqueos.</p>
     *
     * @param organizationId organización propietaria (scope del bloqueo)
     * @param ids identificadores de los artículos referenciados
     * @return estado y unidad base vigentes por artículo, en orden ascendente por {@code id}
     */
    @Query(
            value = "select id as \"id\", status as \"status\", base_unit as \"baseUnit\" "
                    + "from inventory_items "
                    + "where organization_id = :organizationId and id in (:ids) "
                    + "order by id asc for no key update",
            nativeQuery = true)
    List<IssuableItemState> lockIssuableStatesByIdIn(
            @Param("organizationId") UUID organizationId, @Param("ids") Collection<UUID> ids);

    /** Estado vigente de un artículo leído bajo bloqueo, sin pasar por el Persistence Context. */
    interface IssuableItemState {

        UUID getId();

        String getStatus();

        String getBaseUnit();
    }

    /**
     * Marca la unidad base como inmutable sin incrementar {@code @Version} y sin
     * auditar (es parte de la creación de la dependencia que lo motiva).
     *
     * <p>Escribe directamente la columna {@code unit_locked} (mapeada como
     * {@code updatable = false}); es la única sentencia que puede activarla, de
     * modo que el valor es monótono y ninguna actualización de entidad puede
     * revertirlo.</p>
     *
     * @param organizationId organización propietaria
     * @param ids artículos a marcar
     * @return número de filas efectivamente actualizadas
     */
    @Modifying
    @Query("update InventoryItem i set i.unitLocked = true "
            + "where i.organization.id = :organizationId and i.id in :ids and i.unitLocked = false")
    int markUnitLocked(@Param("organizationId") UUID organizationId, @Param("ids") Collection<UUID> ids);

    /**
     * Lee el valor vigente del marcador de unidad base como proyección escalar,
     * de modo que la decisión de mutabilidad no dependa de una instancia
     * administrada potencialmente obsoleta en el Persistence Context.
     *
     * <p>Es una consulta nativa para no disparar el auto-flush de Hibernate: una
     * consulta JPQL forzaría el volcado de los cambios pendientes de la entidad
     * durante el propio {@code update}, provocando un incremento extra de
     * {@code @Version}. Debe invocarse bajo el bloqueo de fila del artículo para
     * leer un valor serializado.</p>
     *
     * @param id artículo
     * @param organizationId organización propietaria
     * @return {@code true} si la unidad está congelada; {@code null} si no existe
     */
    @Query(
            value = "select unit_locked from inventory_items where id = :id and organization_id = :organizationId",
            nativeQuery = true)
    Boolean findUnitLockedFlag(@Param("id") UUID id, @Param("organizationId") UUID organizationId);
}
