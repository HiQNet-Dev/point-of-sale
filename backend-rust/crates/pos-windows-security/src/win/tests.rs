//! Pruebas de custodia con descriptores de seguridad reales de Windows.
//!
//! Cada prueba crea objetos en un directorio temporal e inspecciona el
//! descriptor efectivo que devuelve el sistema. No hay dobles de prueba: si una
//! DACL es compartida, viene del kernel, no de un simulacro.
//!
//! Las comprobaciones son estructurales (propietario y entradas de la DACL) y
//! por tanto independientes de que el proceso se ejecute o no con privilegios
//! elevados. La creación de enlaces simbólicos sí puede requerir privilegios; en
//! ese caso la prueba lo informa en lugar de fingir una comprobación.

use std::{error::Error, io::Write, path::Path};

use crate::{
    SecurityError,
    sddl::{AceEffect, parse_dacl},
};

use super::{
    assert_inside, assert_private, create_private_directory, create_private_file,
    effective_user_sid, native, read_private_file, verify_private_directory, verify_private_file,
};

const AUTHORIZATION: &str = "provisioning-authorization.txt";
const SECRET: &[u8] = b"installation_id=00000000-0000-0000-0000-000000000001\ntoken=no-real\n";

fn failure(reason: &str) -> Box<dyn Error> {
    reason.into()
}

fn owner_sid() -> Result<String, Box<dyn Error>> {
    Ok(effective_user_sid()?)
}

/// Identidades con concesión explícita en el descriptor real del objeto.
fn grant_sids(path: &Path, directory: bool) -> Result<Vec<String>, Box<dyn Error>> {
    let handle = if directory {
        native::open_directory(path)?
    } else {
        native::open_regular_file(path)?
    };
    let (_, sddl) = native::owner_and_sddl(handle.raw())?;
    let aces = parse_dacl(&sddl).map_err(|()| failure("la DACL no pudo interpretarse"))?;
    Ok(aces
        .into_iter()
        .filter(|ace| ace.effect == AceEffect::Grant)
        .map(|ace| ace.sid)
        .collect())
}

fn assert_private_grants(path: &Path, directory: bool) -> Result<(), Box<dyn Error>> {
    let owner = owner_sid()?;
    let grants = grant_sids(path, directory)?;
    let mut expected = vec![
        owner.clone(),
        "S-1-5-18".to_owned(),
        "S-1-5-32-544".to_owned(),
    ];
    let mut actual = grants.clone();
    expected.sort_unstable();
    actual.sort_unstable();
    if actual != expected {
        return Err(failure(&format!(
            "concesiones inesperadas: {actual:?} frente a {expected:?}"
        )));
    }
    Ok(())
}

#[test]
fn exposes_a_canonical_effective_sid() -> Result<(), Box<dyn Error>> {
    let sid = owner_sid()?;
    if !sid.starts_with("S-1-") {
        return Err(failure("el SID efectivo no tiene forma canónica"));
    }
    Ok(())
}

#[test]
fn reports_the_created_descriptor_for_diagnosis() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    let owner = owner_sid()?;
    eprintln!("effective_owner={owner}");
    eprintln!(
        "root_sddl={:?}",
        native::owner_and_sddl(native::open_directory(root.path())?.raw())
    );
    match create_private_directory(&directory) {
        Ok(()) => eprintln!("created=ok"),
        Err(error) => eprintln!("created={error:?}"),
    }
    let handle = native::open_directory(&directory)?;
    let (actual_owner, sddl) = native::owner_and_sddl(handle.raw())?;
    eprintln!("actual_owner={actual_owner}");
    eprintln!("sddl={sddl}");
    eprintln!("parsed={:?}", parse_dacl(&sddl));
    Ok(())
}

#[test]
fn creates_a_private_directory_owned_by_the_effective_user() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");

    create_private_directory(&directory)?;
    verify_private_directory(&directory)?;

    let handle = native::open_directory(&directory)?;
    let (owner, sddl) = native::owner_and_sddl(handle.raw())?;
    if !owner.eq_ignore_ascii_case(&owner_sid()?) {
        return Err(failure("el propietario no es el usuario efectivo"));
    }
    if !sddl.contains("D:P") {
        return Err(failure("la DACL no está protegida frente a la herencia"));
    }
    assert_private_grants(&directory, true)?;
    Ok(())
}

#[test]
fn creates_the_directory_idempotently() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    create_private_directory(&directory)?;
    verify_private_directory(&directory)?;
    Ok(())
}

#[test]
fn rejects_inherited_group_grants() -> Result<(), Box<dyn Error>> {
    // Un directorio ordinario bajo el perfil hereda concesiones para grupos del
    // sistema (por ejemplo, un grupo local y la identidad en la nube del mismo
    // usuario). El producto no las acepta: la instalación se crea con una DACL
    // protegida propia, y una ruta ajena con permisos amplios se rechaza.
    let root = tempfile::tempdir()?;
    let directory = root.path().join("heredada");
    std::fs::create_dir(&directory)?;
    let handle = native::open_directory(&directory)?;
    let (_, sddl) = native::owner_and_sddl(handle.raw())?;
    let owner = owner_sid()?;
    let shared: Vec<String> = parse_dacl(&sddl)
        .map_err(|()| failure("la DACL no pudo interpretarse"))?
        .into_iter()
        .filter(|ace| ace.effect == AceEffect::Grant)
        .map(|ace| ace.sid)
        .filter(|sid| !crate::sddl::is_allowed_identity(sid, &owner))
        .collect();
    if shared.is_empty() {
        // Un perfil sin grupos heredados no aporta evidencia; el caso contrario
        // sí debe rechazarse.
        verify_private_directory(&directory)?;
        return Ok(());
    }
    match verify_private_directory(&directory) {
        Err(SecurityError::InsecureDacl) => Ok(()),
        other => Err(failure(&format!(
            "se esperaba InsecureDacl, llegó {other:?}"
        ))),
    }
}

#[test]
fn rejects_a_directory_with_a_shared_dacl() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let shared = root.path().join("compartida");
    // `WD` es `Everyone`: una concesión real, no simulada.
    native::create_directory_with(&shared, "D:(A;;FA;;;WD)")?;
    match verify_private_directory(&shared) {
        Err(SecurityError::InsecureDacl) => Ok(()),
        other => Err(failure(&format!(
            "se esperaba InsecureDacl, llegó {other:?}"
        ))),
    }
}

#[test]
fn writes_an_exclusive_private_file() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;

    create_private_file(&directory, AUTHORIZATION, SECRET)?;
    let contents = read_private_file(&directory, AUTHORIZATION)?;
    if contents != SECRET {
        return Err(failure("el contenido leído no coincide"));
    }
    verify_private_file(&directory, AUTHORIZATION)?;
    assert_private_grants(&directory.join(AUTHORIZATION), false)?;

    match create_private_file(&directory, AUTHORIZATION, SECRET) {
        Err(SecurityError::AlreadyExists) => Ok(()),
        other => Err(failure(&format!(
            "se esperaba AlreadyExists, llegó {other:?}"
        ))),
    }
}

#[test]
fn rejects_a_file_with_a_shared_dacl() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    let path = directory.join(AUTHORIZATION);
    native::create_file_with(&path, "D:(A;;FA;;;WD)")?;

    for outcome in [
        verify_private_file(&directory, AUTHORIZATION),
        read_private_file(&directory, AUTHORIZATION).map(|_| ()),
        create_private_file(&directory, AUTHORIZATION, SECRET),
    ] {
        match outcome {
            Err(SecurityError::InsecureDacl | SecurityError::AlreadyExists) => {}
            other => return Err(failure(&format!("se esperaba un rechazo, llegó {other:?}"))),
        }
    }
    Ok(())
}

#[test]
fn rejects_a_descriptor_whose_owner_is_not_the_expected_identity() -> Result<(), Box<dyn Error>> {
    // No se puede cambiar el propietario sin elevación; se comprueba la regla
    // contra el descriptor real comparándolo con una identidad distinta.
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    create_private_file(&directory, AUTHORIZATION, SECRET)?;

    let handle = native::open_regular_file(&directory.join(AUTHORIZATION))?;
    // `SYSTEM` no puede ser propietario de una instalación de usuario.
    match assert_private(handle.raw(), "S-1-5-18") {
        Err(SecurityError::WrongOwner) => Ok(()),
        other => Err(failure(&format!("se esperaba WrongOwner, llegó {other:?}"))),
    }
}

#[test]
fn creates_the_object_under_the_effective_owner_even_when_elevated() -> Result<(), Box<dyn Error>> {
    // El propietario se declara en el descriptor: un proceso elevado no hereda
    // el propietario por defecto `Administrators`.
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    create_private_file(&directory, AUTHORIZATION, SECRET)?;

    let handle = native::open_regular_file(&directory.join(AUTHORIZATION))?;
    let (owner, _) = native::owner_and_sddl(handle.raw())?;
    if !owner.eq_ignore_ascii_case(&owner_sid()?) {
        return Err(failure("el archivo no quedó a nombre del usuario efectivo"));
    }
    Ok(())
}

#[test]
fn rejects_a_missing_file_and_directory() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let missing = root.path().join("no-existe");
    match verify_private_directory(&missing) {
        Err(SecurityError::Missing) => {}
        other => return Err(failure(&format!("se esperaba Missing, llegó {other:?}"))),
    }
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    match read_private_file(&directory, AUTHORIZATION) {
        Err(SecurityError::Missing) => Ok(()),
        other => Err(failure(&format!("se esperaba Missing, llegó {other:?}"))),
    }
}

#[test]
fn rejects_a_directory_in_place_of_a_file() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    std::fs::create_dir(directory.join(AUTHORIZATION))?;
    match read_private_file(&directory, AUTHORIZATION) {
        Err(SecurityError::NotExpectedType | SecurityError::Redirect) => Ok(()),
        other => Err(failure(&format!(
            "se esperaba NotExpectedType, llegó {other:?}"
        ))),
    }
}

#[test]
fn rejects_names_that_escape_the_directory() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    for name in [
        "..",
        "../secreto",
        "sub/archivo",
        "sub\\archivo",
        "C:\\Windows\\x",
    ] {
        match read_private_file(&directory, name) {
            Err(SecurityError::InvalidName) => {}
            other => {
                return Err(failure(&format!(
                    "se esperaba InvalidName para {name}, llegó {other:?}"
                )));
            }
        }
    }
    Ok(())
}

#[test]
fn refuses_to_write_into_a_shared_directory() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let shared = root.path().join("compartida");
    native::create_directory_with(&shared, "D:(A;;FA;;;WD)")?;
    match create_private_file(&shared, AUTHORIZATION, SECRET) {
        Err(SecurityError::InsecureDacl) => {
            if shared.join(AUTHORIZATION).exists() {
                return Err(failure(
                    "se escribió el secreto en un directorio compartido",
                ));
            }
            Ok(())
        }
        other => Err(failure(&format!(
            "se esperaba InsecureDacl, llegó {other:?}"
        ))),
    }
}

#[test]
fn only_one_concurrent_creation_succeeds() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;

    let attempts: Vec<Result<(), SecurityError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let directory = directory.clone();
                scope.spawn(move || create_private_file(&directory, AUTHORIZATION, SECRET))
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .collect()
    });

    let created = attempts.iter().filter(|attempt| attempt.is_ok()).count();
    if created != 1 || attempts.len() != 4 {
        return Err(failure(&format!(
            "se esperaba una creación única; {created} de {}",
            attempts.len()
        )));
    }
    read_private_file(&directory, AUTHORIZATION)?;
    Ok(())
}

#[test]
fn rejects_a_windows_junction() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let target = root.path().join("real");
    create_private_directory(&target)?;
    create_private_file(&target, AUTHORIZATION, SECRET)?;
    let junction = root.path().join("enlace");
    if !link_directory(&junction, &target)? {
        eprintln!("el entorno no permite crear junctions; comprobación no ejecutada");
        return Ok(());
    }

    for outcome in [
        verify_private_directory(&junction),
        read_private_file(&junction, AUTHORIZATION).map(|_| ()),
    ] {
        match outcome {
            Err(SecurityError::Redirect) => {}
            other => return Err(failure(&format!("se esperaba Redirect, llegó {other:?}"))),
        }
    }
    Ok(())
}

#[test]
fn rejects_a_symlinked_file_inside_the_installation() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    let target = directory.join("real.txt");
    create_private_file(&directory, "real.txt", SECRET)?;
    let link = directory.join(AUTHORIZATION);
    if std::os::windows::fs::symlink_file(&target, &link).is_err() {
        eprintln!(
            "el entorno no permite crear enlaces simbólicos sin elevación; comprobación no ejecutada"
        );
        return Ok(());
    }

    match read_private_file(&directory, AUTHORIZATION) {
        Err(SecurityError::Redirect) => Ok(()),
        other => Err(failure(&format!("se esperaba Redirect, llegó {other:?}"))),
    }
}

#[test]
fn rejects_a_file_outside_the_validated_directory() -> Result<(), Box<dyn Error>> {
    // La ruta final debe colgar del directorio validado: un nombre que resuelva
    // fuera se considera redirección.
    let root = tempfile::tempdir()?;
    let directory = root.path().join("com.hiqnet.pos");
    create_private_directory(&directory)?;
    let elsewhere = root.path().join("otro");
    create_private_directory(&elsewhere)?;
    create_private_file(&elsewhere, AUTHORIZATION, SECRET)?;

    let directory_handle = native::open_directory(&directory)?;
    let file_handle = native::open_regular_file(&elsewhere.join(AUTHORIZATION))?;
    match assert_inside(directory_handle.raw(), file_handle.raw(), AUTHORIZATION) {
        Err(SecurityError::Redirect) => Ok(()),
        other => Err(failure(&format!("se esperaba Redirect, llegó {other:?}"))),
    }
}

/// Crea una junction; devuelve `false` si el entorno no lo permite.
fn link_directory(link: &Path, target: &Path) -> Result<bool, Box<dyn Error>> {
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()?;
    if output.status.success() {
        return Ok(true);
    }
    std::io::stderr().write_all(&output.stderr)?;
    Ok(false)
}
