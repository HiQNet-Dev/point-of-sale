//! Lectura de la DACL a partir de la representación SDDL del descriptor.
//!
//! El adaptador Windows convierte el descriptor real del objeto a SDDL con
//! `ConvertSecurityDescriptorToStringSecurityDescriptorW` y lo interpreta aquí
//! en Rust seguro. Separar la interpretación del FFI permite probar la política
//! en cualquier plataforma.
//!
//! Formato de una entrada: `(tipo;banderas;derechos;guid_objeto;guid_heredado;cuenta)`.
//! Las secciones del descriptor son `O:` (propietario), `G:` (grupo), `D:`
//! (DACL) y `S:` (SACL), en este orden; ningún valor contiene `:`.

/// Efecto de una entrada de la DACL sobre el acceso.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AceEffect {
    /// Concede acceso: la identidad debe estar autorizada.
    Grant,
    /// Deniega, audita o etiqueta: no amplía el acceso.
    Other,
}

/// Entrada interpretada de la DACL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DaclAce {
    /// Efecto de la entrada.
    pub effect: AceEffect,
    /// Identidad normalizada a su forma `S-1-...` cuando es un alias conocido.
    pub sid: String,
}

/// Identidades a las que la política permite conceder acceso.
///
/// `S-1-3-0` y `S-1-3-4` se resuelven al propietario del objeto cuando Windows
/// propaga entradas heredadas, por lo que se aceptan como equivalentes.
const SHARED_ALLOWED: [&str; 4] = ["S-1-5-18", "S-1-5-32-544", "S-1-3-0", "S-1-3-4"];

/// Indica si la identidad puede recibir una concesión explícita.
pub(crate) fn is_allowed_identity(sid: &str, owner_sid: &str) -> bool {
    sid.eq_ignore_ascii_case(owner_sid)
        || SHARED_ALLOWED
            .iter()
            .any(|allowed| sid.eq_ignore_ascii_case(allowed))
}

/// Extrae e interpreta la DACL del descriptor SDDL.
///
/// # Errores
/// Devuelve `Err(())` cuando la DACL es ausente (`NO_ACCESS_CONTROL`), está mal
/// formada o contiene una entrada cuyo efecto no podemos clasificar. El
/// llamador traduce cualquier error a una denegación: nunca se asume que un
/// descriptor ilegible es privado.
pub(crate) fn parse_dacl(sddl: &str) -> Result<Vec<DaclAce>, ()> {
    let section = dacl_section(sddl).ok_or(())?;
    if section.contains("NO_ACCESS_CONTROL") {
        return Err(());
    }
    let mut aces = Vec::new();
    let mut remaining = section;
    while let Some(start) = remaining.find('(') {
        let rest = &remaining[start + 1..];
        let end = rest.find(')').ok_or(())?;
        aces.push(parse_ace(&rest[..end])?);
        remaining = &rest[end + 1..];
    }
    Ok(aces)
}

fn dacl_section(sddl: &str) -> Option<&str> {
    let start = sddl.find("D:")? + 2;
    let rest = &sddl[start..];
    Some(match rest.find("S:") {
        Some(end) => &rest[..end],
        None => rest,
    })
}

fn parse_ace(entry: &str) -> Result<DaclAce, ()> {
    let fields: Vec<&str> = entry.split(';').collect();
    // Una entrada completa declara seis campos; menos indica truncamiento.
    if fields.len() < 6 {
        return Err(());
    }
    let effect = match fields[0] {
        "A" | "OA" => AceEffect::Grant,
        "D" | "OD" | "AU" | "AL" | "ML" | "RA" | "SP" => AceEffect::Other,
        // Un tipo desconocido podría conceder acceso: se rechaza el descriptor.
        _ => return Err(()),
    };
    let account = fields[5];
    if account.is_empty() {
        return Err(());
    }
    Ok(DaclAce {
        effect,
        sid: normalize_sid(account),
    })
}

/// Traduce los alias SDDL habituales a su forma canónica `S-1-...`.
///
/// Un alias no reconocido se conserva tal cual: al no coincidir con la lista de
/// identidades autorizadas, una concesión con ese alias queda rechazada.
fn normalize_sid(account: &str) -> String {
    if account.len() > 2 && account.starts_with("S-1-") {
        return account.to_ascii_uppercase();
    }
    let canonical = match account {
        "SY" => "S-1-5-18",
        "BA" => "S-1-5-32-544",
        "CO" => "S-1-3-0",
        "OW" => "S-1-3-4",
        "WD" => "S-1-1-0",
        "AU" => "S-1-5-11",
        "BU" => "S-1-5-32-545",
        "BG" => "S-1-5-32-546",
        "PU" => "S-1-5-32-547",
        "IU" => "S-1-5-4",
        "SU" => "S-1-5-6",
        "AN" => "S-1-5-7",
        "LS" => "S-1-5-19",
        "NS" => "S-1-5-20",
        "NU" => "S-1-5-2",
        "RC" => "S-1-5-12",
        "RD" => "S-1-5-32-555",
        "ED" => "S-1-5-9",
        "AG" => "S-1-5-32-551",
        "SO" => "S-1-5-32-549",
        "RE" => "S-1-5-32-552",
        other => other,
    };
    canonical.to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{AceEffect, DaclAce, is_allowed_identity, normalize_sid, parse_dacl};

    const OWNER: &str = "S-1-5-21-1111111111-2222222222-3333333333-1001";

    fn grant(sid: &str) -> DaclAce {
        DaclAce {
            effect: AceEffect::Grant,
            sid: sid.to_owned(),
        }
    }

    #[test]
    fn reads_a_private_descriptor() -> Result<(), Box<dyn std::error::Error>> {
        let sddl =
            format!("O:{OWNER}G:{OWNER}D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;FA;;;{OWNER})");
        assert_eq!(
            parse_dacl(&sddl).map_err(|()| "descriptor ilegible")?,
            vec![grant("S-1-5-18"), grant("S-1-5-32-544"), grant(OWNER)]
        );
        Ok(())
    }

    #[test]
    fn exposes_a_shared_grant() -> Result<(), Box<dyn std::error::Error>> {
        let sddl = format!("O:{OWNER}D:PAI(A;;FA;;;SY)(A;;FA;;;WD)(A;;FA;;;{OWNER})");
        let aces = parse_dacl(&sddl).map_err(|()| "descriptor ilegible")?;
        let shared: Vec<&str> = aces
            .iter()
            .filter(|ace| ace.effect == AceEffect::Grant)
            .map(|ace| ace.sid.as_str())
            .filter(|sid| !is_allowed_identity(sid, OWNER))
            .collect();
        assert_eq!(shared, ["S-1-1-0"]);
        Ok(())
    }

    #[test]
    fn ignores_denials_audits_and_labels() -> Result<(), Box<dyn std::error::Error>> {
        let sddl = format!("O:{OWNER}D:PAI(D;;FR;;;WD)(AU;;FA;;;WD)(ML;;NW;;;HI)(A;;FA;;;{OWNER})");
        let aces = parse_dacl(&sddl).map_err(|()| "descriptor ilegible")?;
        assert!(
            aces.iter()
                .filter(|ace| ace.effect == AceEffect::Grant)
                .all(|ace| is_allowed_identity(&ace.sid, OWNER))
        );
        Ok(())
    }

    #[test]
    fn rejects_missing_protected_or_malformed_dacls() {
        assert_eq!(parse_dacl(&format!("O:{OWNER}G:{OWNER}")), Err(()));
        assert_eq!(
            parse_dacl(&format!("O:{OWNER}D:NO_ACCESS_CONTROL")),
            Err(())
        );
        assert_eq!(parse_dacl(&format!("O:{OWNER}D:(A;;FA;;;SY")), Err(()));
        assert_eq!(
            parse_dacl(&format!("O:{OWNER}D:(A;;FA;;;SY)(XA;;FA;;;WD)")),
            Err(())
        );
        assert_eq!(parse_dacl(&format!("O:{OWNER}D:(A;;FA)")), Err(()));
        assert_eq!(parse_dacl(&format!("O:{OWNER}D:(A;;FA;;;)")), Err(()));
    }

    #[test]
    fn separates_the_sacl_section() -> Result<(), Box<dyn std::error::Error>> {
        let sddl = format!("O:{OWNER}D:P(A;;FA;;;{OWNER})S:(AU;SA;FA;;;WD)");
        let aces = parse_dacl(&sddl).map_err(|()| "descriptor ilegible")?;
        assert_eq!(aces, vec![grant(OWNER)]);
        Ok(())
    }

    #[test]
    fn normalizes_aliases_and_keeps_unknown_ones() {
        assert_eq!(normalize_sid("SY"), "S-1-5-18");
        assert_eq!(normalize_sid("these"), "THESE");
        assert_eq!(normalize_sid("s-1-5-21-1-2-3-1001"), "S-1-5-21-1-2-3-1001");
        assert_eq!(normalize_sid(OWNER), OWNER);
    }

    #[test]
    fn allows_only_the_owner_system_and_administrators() {
        for allowed in [OWNER, "S-1-5-18", "S-1-5-32-544", "S-1-3-0", "S-1-3-4"] {
            assert!(is_allowed_identity(allowed, OWNER), "{allowed}");
        }
        for rejected in ["S-1-1-0", "S-1-5-11", "S-1-5-32-545", "S-1-5-4", "THESE"] {
            assert!(!is_allowed_identity(rejected, OWNER), "{rejected}");
        }
    }
}
