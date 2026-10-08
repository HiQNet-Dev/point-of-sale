package com.hiqnet.pos;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.application.UpdateBranchCommand;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.organization.application.OrganizationService;
import com.hiqnet.pos.organization.application.UpdateOrganizationCommand;
import com.hiqnet.pos.organization.domain.Organization;
import jakarta.persistence.EntityManager;
import jakarta.persistence.EntityManagerFactory;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.context.annotation.Import;
import org.springframework.test.context.ActiveProfiles;

/**
 * Optimistic locking (@Version) detection: two persistence contexts working
 * from the same version cannot both write — the second commit must fail.
 * GlobalExceptionHandler maps the failure to 409 CONCURRENT_MODIFICATION
 * (covered by ConcurrentModificationHandlerTest).
 */
@ActiveProfiles("test")
@Import(TestcontainersConfiguration.class)
@SpringBootTest
class OptimisticLockingTests {

    @Autowired
    private OrganizationService organizationService;

    @Autowired
    private BranchService branchService;

    @Autowired
    private EntityManagerFactory entityManagerFactory;

    @Test
    void organizationUpdateOnStaleVersionFails() {
        Organization created = organizationService.create("Org", "lock-org", "UTC", "MXN", "es-MX");

        EntityManager winner = entityManagerFactory.createEntityManager();
        EntityManager loser = entityManagerFactory.createEntityManager();
        try {
            winner.getTransaction().begin();
            loser.getTransaction().begin();
            Organization winnerView = winner.find(Organization.class, created.getId());
            Organization loserView = loser.find(Organization.class, created.getId());
            winnerView.updateName("Winner");
            winner.getTransaction().commit();

            loserView.updateName("Loser");
            jakarta.persistence.RollbackException ex = assertThrows(
                    jakarta.persistence.RollbackException.class, loser.getTransaction()::commit);
            assertTrue(ex.getCause() instanceof jakarta.persistence.OptimisticLockException);

            Organization current = organizationService.get(created.getId());
            assertEquals("Winner", current.getName());
            assertEquals(1, current.getVersion());
        } finally {
            closeQuietly(winner);
            closeQuietly(loser);
        }
    }

    @Test
    void branchUpdateOnStaleVersionFails() {
        Organization org = organizationService.create("Org", "lock-branch-org", "UTC", "MXN", "es-MX");
        Branch created = branchService.create(org.getId(), "Sucursal", "centro", null);

        EntityManager winner = entityManagerFactory.createEntityManager();
        EntityManager loser = entityManagerFactory.createEntityManager();
        try {
            winner.getTransaction().begin();
            loser.getTransaction().begin();
            Branch winnerView = winner.find(Branch.class, created.getId());
            Branch loserView = loser.find(Branch.class, created.getId());
            winnerView.updateName("Winner");
            winner.getTransaction().commit();

            loserView.updateName("Loser");
            jakarta.persistence.RollbackException ex = assertThrows(
                    jakarta.persistence.RollbackException.class, loser.getTransaction()::commit);
            assertTrue(ex.getCause() instanceof jakarta.persistence.OptimisticLockException);

            Branch current = branchService.get(org.getId(), created.getId());
            assertEquals("Winner", current.getName());
            assertEquals(1, current.getVersion());
        } finally {
            closeQuietly(winner);
            closeQuietly(loser);
        }
    }

    @Test
    void sequentialUpdatesSucceedAndBumpVersion() {
        Organization created = organizationService.create("Org", "seq-lock", "UTC", "MXN", "es-MX");
        Organization v1 = organizationService.update(
                created.getId(), new UpdateOrganizationCommand("New Name", null, null, null, null));
        assertEquals(1, v1.getVersion());
        Organization v2 = organizationService.update(
                created.getId(), new UpdateOrganizationCommand("Newer Name", null, null, null, null));
        assertEquals(2, v2.getVersion());
    }

    private static void closeQuietly(EntityManager entityManager) {
        if (entityManager != null && entityManager.isOpen()) {
            if (entityManager.getTransaction().isActive()) {
                entityManager.getTransaction().rollback();
            }
            entityManager.close();
        }
    }
}