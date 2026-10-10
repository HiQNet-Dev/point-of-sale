package com.hiqnet.pos.cashregister.domain;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import org.junit.jupiter.api.Test;

/**
 * Pure domain invariants for the branch-owned cash register (no Spring).
 */
class CashRegisterDomainTest {

    private static final Organization ORG = Organization.create("Org", "org", "UTC", "MXN", "es-MX");
    private static final Branch BRANCH = Branch.create(ORG, "Centro", "centro", null);

    @Test
    void createStartsActiveWithZeroVersionAndTrimsName() {
        CashRegister register = CashRegister.create(ORG, BRANCH, "register-1", "  Front Desk  ");

        assertThat(register.getId()).isNotNull();
        assertThat(register.getOrganization()).isSameAs(ORG);
        assertThat(register.getBranch()).isSameAs(BRANCH);
        assertThat(register.getCode()).isEqualTo("register-1");
        assertThat(register.getName()).isEqualTo("Front Desk");
        assertThat(register.getStatus()).isEqualTo(CashRegisterStatus.ACTIVE);
        assertThat(register.getVersion()).isZero();
        assertThat(register.isActive()).isTrue();
    }

    @Test
    void createRejectsBlankName() {
        assertThatThrownBy(() -> CashRegister.create(ORG, BRANCH, "register-1", "   "))
                .isInstanceOf(BusinessException.class)
                .hasMessageContaining("name is required");
    }

    @Test
    void createRejectsNameLongerThan120() {
        String tooLong = "a".repeat(121);
        assertThatThrownBy(() -> CashRegister.create(ORG, BRANCH, "register-1", tooLong))
                .isInstanceOf(BusinessException.class)
                .hasMessageContaining("name must not exceed 120 characters");
    }

    @Test
    void createRejectsNonSlugCode() {
        assertThatThrownBy(() -> CashRegister.create(ORG, BRANCH, "Register 1", "Front"))
                .isInstanceOf(BusinessException.class)
                .hasMessageContaining("code must match");
    }

    @Test
    void createRejectsCodeLongerThan64() {
        String tooLong = "a".repeat(65);
        assertThatThrownBy(() -> CashRegister.create(ORG, BRANCH, tooLong, "Front"))
                .isInstanceOf(BusinessException.class)
                .hasMessageContaining("code must not exceed 64 characters");
    }

    @Test
    void updateNameTrimsAndRejectsBlank() {
        CashRegister register = CashRegister.create(ORG, BRANCH, "register-1", "Front");
        register.updateName("  Desk  ");
        assertThat(register.getName()).isEqualTo("Desk");
        assertThatThrownBy(() -> register.updateName(" "))
                .isInstanceOf(BusinessException.class);
    }

    @Test
    void updateStatusIgnoresNullAndChangesStatus() {
        CashRegister register = CashRegister.create(ORG, BRANCH, "register-1", "Front");
        register.updateStatus(null);
        assertThat(register.getStatus()).isEqualTo(CashRegisterStatus.ACTIVE);
        register.updateStatus(CashRegisterStatus.INACTIVE);
        assertThat(register.getStatus()).isEqualTo(CashRegisterStatus.INACTIVE);
        assertThat(register.isActive()).isFalse();
    }
}
