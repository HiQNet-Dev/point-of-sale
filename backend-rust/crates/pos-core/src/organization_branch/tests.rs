use super::*;

fn entity(value: u128) -> Result<EntityId, DomainError> {
    EntityId::parse(&format!("00000000-0000-0000-0000-{value:012x}"))
}

fn organization_at(revision: u64) -> Result<Organization, DomainError> {
    Ok(Organization::rehydrate(
        entity(1)?,
        DisplayName::new("Cafetería Central")?,
        BusinessCode::new("cafeteria-central")?,
        Timezone::new("America/Mexico_City")?,
        IsoCurrency::new("MXN")?,
        LocaleTag::new("es-MX")?,
        Status::Active,
        NegativeStockPolicy::Strict,
        revision,
    ))
}

#[test]
fn timezone_accepts_java_zone_id_fixed_offset_forms() -> Result<(), DomainError> {
    for sign in ['+', '-'] {
        for suffix in ["1", "01", "0130", "01:30", "013015", "01:30:15"] {
            Timezone::new(&format!("{sign}{suffix}"))?;
            for prefix in ["UTC", "GMT", "UT"] {
                Timezone::new(&format!("{prefix}{sign}{suffix}"))?;
            }
        }
    }
    for zone in [
        "Z",
        "UTC",
        "GMT",
        "UT",
        "+18",
        "+1800",
        "+180000",
        "-18:00",
        "UTC+05:30",
        "America/Mexico_City",
        "America/Argentina/Buenos_Aires",
        "Etc/GMT+5",
    ] {
        Timezone::new(zone)?;
    }
    Ok(())
}

#[test]
fn timezone_rejects_out_of_range_and_unknown_identifiers() {
    for zone in [
        "",
        " ",
        "+",
        "+001",
        "+1:30",
        "+01:3",
        "+01:30:1",
        "+01:30:15:00",
        "+18:01",
        "-18:01",
        "+19",
        "+19:00",
        "UTC+18:01",
        "+01:60",
        "+01:00:60",
        "+00:99",
        "Europe/",
        "America//Mexico",
        "Not/AZone",
        "UTCZ",
        "GMTZ",
        "UTZ",
        "+€a",
    ] {
        assert_eq!(
            Timezone::new(zone),
            Err(DomainError::InvalidTimezone),
            "zona {zone}"
        );
    }
    assert_eq!(
        Timezone::new(&"x".repeat(65)),
        Err(DomainError::InvalidTimezone)
    );
}

#[test]
fn iso_currency_matches_the_reference_jdk_catalog() -> Result<(), DomainError> {
    for code in ["USD", "MXN", "eur", "XTS", "ADP"] {
        IsoCurrency::new(code)?;
    }
    for code in ["", "EU", "ZZZ", "usd ", "€UR", "12$"] {
        assert_eq!(
            IsoCurrency::new(code),
            Err(DomainError::InvalidCurrency),
            "moneda {code}"
        );
    }
    // `binary_search` exige el catálogo ordenado; el test evita que una edición
    // posterior silencie códigos válidos.
    assert!(
        JAVA_CURRENCIES.windows(2).all(|pair| pair[0] < pair[1]),
        "el catálogo ISO 4217 debe permanecer ordenado"
    );
    Ok(())
}

#[test]
fn locale_tag_requires_canonical_bcp47() -> Result<(), DomainError> {
    for tag in ["es-MX", "en-US", "es-MX-u-ca-gregory", "zh-Hant-TW"] {
        LocaleTag::new(tag)?;
    }
    for tag in ["", "es_MX", "e", "es--MX", "es-こんにちは"] {
        assert_eq!(
            LocaleTag::new(tag),
            Err(DomainError::InvalidLocale),
            "locale {tag}"
        );
    }
    assert_eq!(
        LocaleTag::new(&"x".repeat(21)),
        Err(DomainError::InvalidLocale)
    );
    Ok(())
}

#[test]
fn organization_patch_reports_stable_changed_fields() -> Result<(), DomainError> {
    let mut organization = organization_at(0)?;
    let changed = organization.patch(OrganizationPatch {
        expected_revision: 0,
        name: Some(DisplayName::new("Café Renovado")?),
        timezone: None,
        currency: None,
        locale: None,
        status: Some(Status::Inactive),
        negative_stock_policy: Some(NegativeStockPolicy::AllowNegative),
    })?;
    assert!(changed);
    assert_eq!(organization.revision(), 1);
    assert_eq!(organization.status(), Status::Inactive);
    assert_eq!(
        organization_changed_fields(&organization_at(0)?, &organization),
        ["name", "status", "inventoryNegativeStockPolicy"]
    );
    Ok(())
}

#[test]
fn no_material_change_keeps_the_revision_and_stale_reads_fail() -> Result<(), DomainError> {
    let mut organization = organization_at(5)?;
    let empty = OrganizationPatch {
        expected_revision: 5,
        name: None,
        timezone: None,
        currency: None,
        locale: None,
        status: None,
        negative_stock_policy: None,
    };
    assert!(!organization.patch(empty)?);
    assert_eq!(organization.revision(), 5);

    let before = organization.clone();
    assert_eq!(
        organization.patch(OrganizationPatch {
            expected_revision: 4,
            name: Some(DisplayName::new("No debe persistir")?),
            timezone: None,
            currency: None,
            locale: None,
            status: None,
            negative_stock_policy: None,
        }),
        Err(DomainError::StaleRevision)
    );
    assert_eq!(organization, before);
    Ok(())
}

#[test]
fn organization_rejects_revision_increment_beyond_i64_max() -> Result<(), DomainError> {
    assert_eq!(MAX_REVISION, i64::MAX as u64);
    let mut organization = organization_at(MAX_REVISION)?;
    let before = organization.clone();
    assert_eq!(
        organization.patch(OrganizationPatch {
            expected_revision: MAX_REVISION,
            name: Some(DisplayName::new("No debe persistir")?),
            timezone: None,
            currency: None,
            locale: None,
            status: None,
            negative_stock_policy: None,
        }),
        Err(DomainError::RevisionOverflow)
    );
    assert_eq!(organization, before);
    Ok(())
}

#[test]
fn branch_inherits_timezone_and_rejects_a_foreign_owner() -> Result<(), DomainError> {
    let organization = organization_at(0)?;
    let mut branch = Branch::create(
        entity(2)?,
        organization.id().clone(),
        NewBranch {
            name: DisplayName::new("Sucursal Centro")?,
            code: BusinessCode::new("centro")?,
            timezone_override: None,
        },
    );
    assert_eq!(
        branch.effective_timezone(organization.timezone()),
        organization.timezone()
    );

    assert_eq!(
        branch.patch(
            &entity(3)?,
            BranchPatch {
                expected_revision: 0,
                name: None,
                timezone_override: TimezoneOverridePatch::Keep,
                status: None,
            },
        ),
        Err(DomainError::OwnershipMismatch)
    );

    let override_zone = Timezone::new("America/Argentina/Buenos_Aires")?;
    assert!(branch.patch(
        organization.id(),
        BranchPatch {
            expected_revision: 0,
            name: None,
            timezone_override: TimezoneOverridePatch::Set(override_zone.clone()),
            status: None,
        },
    )?);
    assert_eq!(branch.revision(), 1);
    assert_eq!(
        branch.effective_timezone(organization.timezone()),
        &override_zone
    );

    assert!(branch.patch(
        organization.id(),
        BranchPatch {
            expected_revision: 1,
            name: None,
            timezone_override: TimezoneOverridePatch::Clear,
            status: None,
        },
    )?);
    assert_eq!(
        branch.effective_timezone(organization.timezone()),
        organization.timezone()
    );
    Ok(())
}

#[test]
fn branch_rejects_revision_increment_beyond_i64_max() -> Result<(), DomainError> {
    let organization = organization_at(0)?;
    let mut branch = Branch::rehydrate(
        entity(2)?,
        organization.id().clone(),
        DisplayName::new("Sucursal Centro")?,
        BusinessCode::new("centro")?,
        None,
        Status::Active,
        MAX_REVISION,
    );
    let before = branch.clone();
    assert_eq!(
        branch.patch(
            organization.id(),
            BranchPatch {
                expected_revision: MAX_REVISION,
                name: None,
                timezone_override: TimezoneOverridePatch::Keep,
                status: Some(Status::Inactive),
            },
        ),
        Err(DomainError::RevisionOverflow)
    );
    assert_eq!(branch, before);
    Ok(())
}
