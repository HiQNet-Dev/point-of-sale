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
    /// Cuenta tal como la escribe el descriptor: un SID canónico o un alias
    /// SDDL del tipo `LA`, `BA` o `SY`.
    ///
    /// El alias no se interpreta aquí porque algunos —como `LA`, el
    /// administrador integrado— dependen de la máquina. La resolución a SID
    /// canónico la hace el adaptador nativo, que es el único que puede hacerlo,
    /// y una cuenta que no se pueda resolver se rechaza.
    pub account: String,
}

/// Grupo de administradores locales.
const ADMINISTRATORS: &str = "S-1-5-32-544";

/// Identidades a las que la política permite conceder acceso.
///
/// `S-1-3-0` y `S-1-3-4` se resuelven al propietario del objeto cuando Windows
/// propaga entradas heredadas, por lo que se aceptan como equivalentes.
const SHARED_ALLOWED: [&str; 4] = ["S-1-5-18", ADMINISTRATORS, "S-1-3-0", "S-1-3-4"];

/// Indica si la identidad puede recibir una concesión explícita.
pub(crate) fn is_allowed_identity(sid: &str, owner_sid: &str) -> bool {
    sid.eq_ignore_ascii_case(owner_sid)
        || SHARED_ALLOWED
            .iter()
            .any(|allowed| sid.eq_ignore_ascii_case(allowed))
}

/// Indica si la identidad puede ser propietaria del objeto.
///
/// Se admite el usuario efectivo y el grupo `Administrators`. Un proceso
/// elevado crea objetos cuyo propietario por defecto es `Administrators`, y ese
/// grupo puede tomar posesión de cualquier archivo de todos modos, así que
/// aceptarlo no amplía el acceso real. Cualquier otra identidad —otro usuario,
/// `SYSTEM`, un grupo ajeno— se rechaza.
pub(crate) fn is_acceptable_owner(sid: &str, effective_sid: &str) -> bool {
    sid.eq_ignore_ascii_case(effective_sid) || sid.eq_ignore_ascii_case(ADMINISTRATORS)
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
        account: account.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{AceEffect, DaclAce, is_acceptable_owner, is_allowed_identity, parse_dacl};

    const OWNER: &str = "S-1-5-21-1111111111-2222222222-3333333333-1001";

    fn grant(account: &str) -> DaclAce {
        DaclAce {
            effect: AceEffect::Grant,
            account: account.to_owned(),
        }
    }

    fn granted(sddl: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        Ok(parse_dacl(sddl)
            .map_err(|()| "descriptor ilegible")?
            .into_iter()
            .filter(|ace| ace.effect == AceEffect::Grant)
            .map(|ace| ace.account)
            .collect())
    }

    #[test]
    fn reads_a_private_descriptor() -> Result<(), Box<dyn std::error::Error>> {
        let sddl =
            format!("O:{OWNER}G:{OWNER}D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;FA;;;{OWNER})");
        assert_eq!(
            parse_dacl(&sddl).map_err(|()| "descriptor ilegible")?,
            vec![grant("SY"), grant("BA"), grant(OWNER)]
        );
        Ok(())
    }

    #[test]
    fn exposes_a_shared_grant() -> Result<(), Box<dyn std::error::Error>> {
        // `WD` es `Everyone`: la cuenta se devuelve tal cual y el adaptador la
        // resuelve a su SID, que no estara en la lista de identidades admitidas.
        let sddl = format!("O:{OWNER}D:PAI(A;;FA;;;SY)(A;;FA;;;WD)(A;;FA;;;{OWNER})");
        assert_eq!(granted(&sddl)?, ["SY", "WD", OWNER]);
        Ok(())
    }

    #[test]
    fn ignores_denials_audits_and_labels() -> Result<(), Box<dyn std::error::Error>> {
        let sddl = format!("O:{OWNER}D:PAI(D;;FR;;;WD)(AU;;FA;;;WD)(ML;;NW;;;HI)(A;;FA;;;{OWNER})");
        assert_eq!(granted(&sddl)?, [OWNER]);
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
        assert_eq!(granted(&sddl)?, [OWNER]);
        Ok(())
    }

    #[test]
    fn accepts_only_a_user_or_administrators_owner() {
        assert!(is_acceptable_owner(OWNER, OWNER));
        assert!(is_acceptable_owner("S-1-5-32-544", OWNER));
        for rejected in ["S-1-5-18", "S-1-5-32-545", "S-1-5-21-9-9-9-1002"] {
            assert!(!is_acceptable_owner(rejected, OWNER), "{rejected}");
        }
    }

    #[test]
    fn allows_only_the_owner_system_and_administrators() {
        for allowed in [OWNER, "S-1-5-18", "S-1-5-32-544", "S-1-3-0", "S-1-3-4"] {
            assert!(is_allowed_identity(allowed, OWNER), "{allowed}");
        }
        for rejected in ["S-1-1-0", "S-1-5-11", "S-1-5-32-545", "S-1-5-4", "LA"] {
            assert!(!is_allowed_identity(rejected, OWNER), "{rejected}");
        }
    }
}
