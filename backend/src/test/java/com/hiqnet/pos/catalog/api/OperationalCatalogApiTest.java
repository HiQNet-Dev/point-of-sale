package com.hiqnet.pos.catalog.api;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import com.hiqnet.pos.AbstractRbacIntegrationTest;
import com.hiqnet.pos.branch.domain.Branch;
import com.hiqnet.pos.branch.domain.BranchRepository;
import com.hiqnet.pos.branch.domain.BranchStatus;
import com.hiqnet.pos.catalog.domain.Category;
import com.hiqnet.pos.catalog.domain.CategoryRepository;
import com.hiqnet.pos.catalog.domain.CategoryStatus;
import com.hiqnet.pos.catalog.domain.ModifierGroup;
import com.hiqnet.pos.catalog.domain.ModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ModifierGroupStatus;
import com.hiqnet.pos.catalog.domain.ModifierOption;
import com.hiqnet.pos.catalog.domain.ModifierOptionRepository;
import com.hiqnet.pos.catalog.domain.ModifierOptionStatus;
import com.hiqnet.pos.catalog.domain.Product;
import com.hiqnet.pos.catalog.domain.ProductModifierGroup;
import com.hiqnet.pos.catalog.domain.ProductModifierGroupRepository;
import com.hiqnet.pos.catalog.domain.ProductRepository;
import com.hiqnet.pos.catalog.domain.ProductStatus;
import com.hiqnet.pos.membership.domain.BranchMembership;
import com.hiqnet.pos.membership.domain.BranchMembershipRepository;
import com.hiqnet.pos.membership.domain.BranchMembershipRole;
import com.hiqnet.pos.membership.domain.BranchMembershipRoleRepository;
import com.hiqnet.pos.organization.domain.Organization;
import com.hiqnet.pos.organization.domain.OrganizationRepository;
import java.math.BigDecimal;
import java.util.UUID;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;


class OperationalCatalogApiTest extends AbstractRbacIntegrationTest {

    @Autowired private OrganizationRepository organizationRepository;
    @Autowired private BranchRepository branchRepository;
    @Autowired private BranchMembershipRepository membershipRepository;
    @Autowired private BranchMembershipRoleRepository membershipRoleRepository;
    @Autowired private CategoryRepository categoryRepository;
    @Autowired private ProductRepository productRepository;
    @Autowired private ModifierGroupRepository groupRepository;
    @Autowired private ModifierOptionRepository optionRepository;
    @Autowired private ProductModifierGroupRepository assignmentRepository;

    @Test
    void branchCatalogPermissionDiscoversSellableCatalogWithoutAdminPrivileges() throws Exception {
        RbacFixture fx = fixture();
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        Branch branchA = branch(fx, "pos-a");
        Branch branchB = branch(fx, "pos-b");
        Category inactiveCategory = Category.create(organization, "retired", "Retired category");
        inactiveCategory.updateStatus(CategoryStatus.INACTIVE);
        categoryRepository.saveAndFlush(inactiveCategory);

        Product sellable = Product.create(organization, "ACTIVE-1", "Active coffee", new BigDecimal("10.0000"));
        sellable.assignCategory(inactiveCategory);
        productRepository.saveAndFlush(sellable);
        Product inactive = Product.create(organization, "INACTIVE-1", "Inactive coffee", new BigDecimal("20.0000"));
        inactive.updateStatus(ProductStatus.INACTIVE);
        productRepository.saveAndFlush(inactive);

        ModifierGroup requiredInactive = ModifierGroup.create(organization, "required", "Required", 1, 1);
        requiredInactive.updateStatus(ModifierGroupStatus.INACTIVE);
        groupRepository.saveAndFlush(requiredInactive);
        ModifierOption inactiveOption = ModifierOption.create(
                organization, requiredInactive, "old-choice", "Old choice", new BigDecimal("-1.0000"), 2);
        inactiveOption.updateStatus(ModifierOptionStatus.INACTIVE);
        optionRepository.saveAndFlush(inactiveOption);

        ModifierGroup optionalInactive = ModifierGroup.create(organization, "optional", "Optional", 0, 2);
        optionalInactive.updateStatus(ModifierGroupStatus.INACTIVE);
        groupRepository.saveAndFlush(optionalInactive);
        optionRepository.saveAndFlush(ModifierOption.create(
                organization, optionalInactive, "extra", "Extra", new BigDecimal("2.0000"), 0));
        ModifierGroup unassigned = groupRepository.saveAndFlush(
                ModifierGroup.create(organization, "unassigned", "Unassigned", 0, 1));
        optionRepository.saveAndFlush(ModifierOption.create(
                organization, unassigned, "not-on-product", "Not on product", BigDecimal.ZERO, 0));

        assignmentRepository.saveAllAndFlush(java.util.List.of(
                ProductModifierGroup.of(fx.orgId(), sellable.getId(), requiredInactive.getId(), 7),
                ProductModifierGroup.of(fx.orgId(), sellable.getId(), optionalInactive.getId(), 8)));

        UUID roleId = createRole(fx, "pos-operator", "POS Operator", "pos.catalog.read");
        BranchMembership membership = membershipRepository.saveAndFlush(
                BranchMembership.create(fx.orgId(), branchA.getId(), fx.userId()));
        membershipRoleRepository.saveAndFlush(BranchMembershipRole.of(fx.orgId(), membership.getId(), roleId));
        String token = login(fx);
        String base = "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchA.getId() + "/pos/catalog";

        mockMvc.perform(get(base + "/categories").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].status").value("INACTIVE"));
        mockMvc.perform(get(base + "/products")
                        .param("categoryId", inactiveCategory.getId().toString())
                        .param("status", "INACTIVE")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1))
                .andExpect(jsonPath("$.items[0].id").value(sellable.getId().toString()))
                .andExpect(jsonPath("$.items[0].status").value("ACTIVE"));
        mockMvc.perform(get(base + "/products").param("q", "active-1").header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.totalElements").value(1));
        mockMvc.perform(get(base + "/products/" + sellable.getId() + "/configuration")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.product.unitPrice").value(10.0000))
                .andExpect(jsonPath("$.groups.length()").value(2))
                .andExpect(jsonPath("$.groups[0].minSelections").value(1))
                .andExpect(jsonPath("$.groups[0].status").value("INACTIVE"))
                .andExpect(jsonPath("$.groups[0].displayOrder").value(7))
                .andExpect(jsonPath("$.groups[0].options[0].status").value("INACTIVE"))
                .andExpect(jsonPath("$.groups[0].options[0].priceDelta").value(-1.0000))
                .andExpect(jsonPath("$.groups[1].minSelections").value(0))
                .andExpect(jsonPath("$.groups[1].status").value("INACTIVE"));
        mockMvc.perform(get(base + "/products/" + inactive.getId() + "/configuration")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isConflict())
                .andExpect(jsonPath("$.code").value("PRODUCT_INACTIVE"));

        // Operational access does not grant any administrative catalog permission.
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/categories")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/modifier-groups/"
                        + requiredInactive.getId() + "/options").header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
        mockMvc.perform(org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post(
                                "/api/v1/organizations/" + fx.orgId() + "/products")
                        .header("Authorization", bearer(token))
                        .contentType("application/json")
                        .content("{\"sku\":\"DENIED-1\",\"name\":\"Denied\",\"unitPrice\":\"1.0000\"}"))
                .andExpect(status().isForbidden());
        mockMvc.perform(org.springframework.test.web.servlet.request.MockMvcRequestBuilders.patch(
                                "/api/v1/organizations/" + fx.orgId() + "/products/" + sellable.getId())
                        .header("Authorization", bearer(token))
                        .contentType("application/json")
                        .content("{\"expectedVersion\":0,\"name\":\"Denied\"}"))
                .andExpect(status().isForbidden());
        mockMvc.perform(org.springframework.test.web.servlet.request.MockMvcRequestBuilders.put(
                                "/api/v1/organizations/" + fx.orgId() + "/products/" + sellable.getId()
                                        + "/modifier-groups")
                        .header("Authorization", bearer(token))
                        .contentType("application/json")
                        .content("{\"expectedVersion\":0,\"modifierGroupIds\":[]}"))
                .andExpect(status().isForbidden());

        String otherBranchUrl = "/api/v1/organizations/" + fx.orgId() + "/branches/" + branchB.getId()
                + "/pos/catalog/products";
        mockMvc.perform(get(otherBranchUrl).header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void noOperationalPermissionAndInactiveBranchAreDenied() throws Exception {
        RbacFixture fx = fixture();
        Branch branch = branch(fx, "pos-no-permission");
        BranchMembership membership = membershipRepository.saveAndFlush(
                BranchMembership.create(fx.orgId(), branch.getId(), fx.userId()));
        UUID roleId = createRole(fx, "empty-pos-role", "Empty POS Role", "sale.read");
        membershipRoleRepository.saveAndFlush(BranchMembershipRole.of(fx.orgId(), membership.getId(), roleId));
        String token = login(fx);
        String url = "/api/v1/organizations/" + fx.orgId() + "/branches/" + branch.getId()
                + "/pos/catalog/categories";

        mockMvc.perform(get(url).header("Authorization", bearer(token))).andExpect(status().isForbidden());
        branch.updateStatus(BranchStatus.INACTIVE);
        branchRepository.saveAndFlush(branch);
        UUID operationalRoleId = createRole(fx, "inactive-pos-role", "Inactive POS Role", "pos.catalog.read");
        membershipRoleRepository.saveAndFlush(
                BranchMembershipRole.of(fx.orgId(), membership.getId(), operationalRoleId));
        mockMvc.perform(get(url).header("Authorization", bearer(token))).andExpect(status().isForbidden());
    }

    @Test
    void catalogAuthorizationDoesNotCrossOrganization() throws Exception {
        RbacFixture fx = fixture();
        Branch branch = branch(fx, "pos-tenant-a");
        UUID roleId = createRole(fx, "tenant-pos-catalog", "Tenant POS Catalog", "pos.catalog.read");
        BranchMembership membership = membershipRepository.saveAndFlush(
                BranchMembership.create(fx.orgId(), branch.getId(), fx.userId()));
        membershipRoleRepository.saveAndFlush(BranchMembershipRole.of(fx.orgId(), membership.getId(), roleId));
        String token = login(fx);

        String foreignOrganizationId = createOrganization();
        Organization foreignOrganization = organizationRepository.findById(UUID.fromString(foreignOrganizationId))
                .orElseThrow();
        Branch foreignBranch = branchRepository.saveAndFlush(
                Branch.create(foreignOrganization, "Foreign POS", "foreign-pos", null));
        mockMvc.perform(get("/api/v1/organizations/" + foreignOrganizationId + "/branches/"
                        + foreignBranch.getId() + "/pos/catalog/products")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isForbidden());
    }

    @Test
    void organizationWideOperationalPermissionUsesExistingBranchOverride() throws Exception {
        RbacFixture fx = fixture();
        Branch branch = branch(fx, "pos-org-override");
        createRoleAndAssign(fx, "org-pos-catalog", "Organization POS Catalog", "pos.catalog.read");
        String token = login(fx);
        mockMvc.perform(get("/api/v1/organizations/" + fx.orgId() + "/branches/" + branch.getId()
                        + "/pos/catalog/categories")
                        .header("Authorization", bearer(token)))
                .andExpect(status().isOk());
    }

    private Branch branch(RbacFixture fx, String code) {
        Organization organization = organizationRepository.findById(fx.orgId()).orElseThrow();
        return branchRepository.saveAndFlush(Branch.create(organization, "Branch " + code, code, null));
    }
}
