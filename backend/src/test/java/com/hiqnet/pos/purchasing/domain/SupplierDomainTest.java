package com.hiqnet.pos.purchasing.domain;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.shared.error.BusinessException;
import java.util.UUID;
import org.junit.jupiter.api.Test;

class SupplierDomainTest {

    private static final Organization ORGANIZATION =
            Organization.create("Supplier Test Organization", "supplier-test-org", "UTC", "MXN", "es-MX");

    @Test
    void createNormalizesMutableMasterDataAndStartsActive() {
        Supplier supplier = Supplier.create(
                ORGANIZATION,
                "supplier-1",
                "  Supplier One  ",
                "  Supplier One, S.A.  ",
                "  TAX-001  ",
                "  Ana Contact  ",
                "  SALES@SUPPLIER.EXAMPLE  ",
                "  +52 555 0101  ",
                "  Main Street 1  ");

        assertThat(supplier.getId()).isInstanceOf(UUID.class);
        assertThat(supplier.getName()).isEqualTo("Supplier One");
        assertThat(supplier.getLegalName()).isEqualTo("Supplier One, S.A.");
        assertThat(supplier.getTaxId()).isEqualTo("TAX-001");
        assertThat(supplier.getContactName()).isEqualTo("Ana Contact");
        assertThat(supplier.getEmail()).isEqualTo("sales@supplier.example");
        assertThat(supplier.getPhone()).isEqualTo("+52 555 0101");
        assertThat(supplier.getAddress()).isEqualTo("Main Street 1");
        assertThat(supplier.getStatus()).isEqualTo(SupplierStatus.ACTIVE);
    }

    @Test
    void codeNameAndEmailMustRespectDomainInvariants() {
        assertThatThrownBy(() -> Supplier.create(
                        ORGANIZATION, "Supplier_1", "Supplier", null, null, null, null, null, null))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> Supplier.create(
                        ORGANIZATION, "supplier-1", "   ", null, null, null, null, null, null))
                .isInstanceOf(BusinessException.class);
        assertThatThrownBy(() -> Supplier.create(
                        ORGANIZATION, "supplier-1", "Supplier", null, null, null, "invalid-email", null, null))
                .isInstanceOf(BusinessException.class);
    }

    @Test
    void optionalTextCanBeClearedWithoutChangingTheImmutableCode() {
        Supplier supplier = Supplier.create(
                ORGANIZATION,
                "supplier-1",
                "Supplier",
                "Legal Name",
                "TAX-001",
                "Contact",
                "sales@supplier.example",
                "5550101",
                "Address");

        supplier.updateLegalName("  ");
        supplier.updateTaxId("  ");
        supplier.updateContactName("  ");
        supplier.updateEmail("  ");
        supplier.updatePhone("  ");
        supplier.updateAddress("  ");
        supplier.updateStatus(SupplierStatus.INACTIVE);

        assertThat(supplier.getCode()).isEqualTo("supplier-1");
        assertThat(supplier.getLegalName()).isNull();
        assertThat(supplier.getTaxId()).isNull();
        assertThat(supplier.getContactName()).isNull();
        assertThat(supplier.getEmail()).isNull();
        assertThat(supplier.getPhone()).isNull();
        assertThat(supplier.getAddress()).isNull();
        assertThat(supplier.getStatus()).isEqualTo(SupplierStatus.INACTIVE);
    }
}
