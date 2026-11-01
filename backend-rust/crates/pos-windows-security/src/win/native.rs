//! Envoltorios mínimos sobre las APIs nativas de seguridad de Windows.
//!
//! Aquí —y solo aquí— se usa `unsafe`. Cada bloque documenta el invariante que
//! lo hace correcto: handles propiedad del proceso, memoria liberada con
//! `LocalFree`, búferes alineados para `TOKEN_USER` y SID válidos mientras viva
//! el descriptor que los contiene.
//!
//! El módulo no decide política: solo abre, lee descriptores, crea objetos con
//! una DACL explícita y traduce códigos de error. La política vive en
//! [`super`], que no necesita `unsafe`.

use std::{
    ffi::OsStr,
    io::{Read, Write},
    mem::{ManuallyDrop, size_of},
    os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, RawHandle},
    },
    path::Path,
    ptr,
};

use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, ERROR_FILE_NOT_FOUND,
        ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, GetLastError, HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
        LocalFree,
    },
    Security::{
        Authorization::{
            ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
            ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SDDL_REVISION_1,
            SE_FILE_OBJECT,
        },
        DACL_SECURITY_INFORMATION, GetTokenInformation, OWNER_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    Storage::FileSystem::{
        CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_READ_DATA, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_DATA, FileAttributeTagInfo,
        GetFileInformationByHandleEx, GetFinalPathNameByHandleW, OPEN_EXISTING, READ_CONTROL,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

use crate::SecurityError;

/// Tipo del handle nativo, reexportado para las firmas de [`super`].
pub(super) use windows_sys::Win32::Foundation::HANDLE as Handle;

/// Banderas del descriptor que se leen y se escriben en este módulo.
const DESCRIPTOR_PARTS: u32 = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;

/// Handle propiedad del proceso; se cierra al destruirse y nunca se hereda.
pub(super) struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn adopt(handle: HANDLE) -> Result<Self, SecurityError> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(SecurityError::Native);
        }
        Ok(Self(handle))
    }

    pub(super) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: el handle lo devolvió una llamada nativa correcta y no se
        // clona ni se transfiere a ningún otro propietario.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Bloque de memoria devuelto por Win32 con `LocalAlloc`, liberado con
/// `LocalFree` al destruirse.
struct LocalMemory(HLOCAL);

impl Drop for LocalMemory {
    fn drop(&mut self) {
        // SAFETY: el puntero proviene de una API que documenta `LocalFree` y no
        // se libera en ningún otro punto.
        unsafe {
            LocalFree(self.0);
        }
    }
}

/// Atributos reales del objeto al que apunta un handle.
pub(super) struct Attributes {
    flags: u32,
}

impl Attributes {
    pub(super) fn is_reparse_point(&self) -> bool {
        self.flags & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }

    pub(super) fn is_directory(&self) -> bool {
        self.flags & FILE_ATTRIBUTE_DIRECTORY != 0
    }
}

fn wide(value: &OsStr) -> Result<Vec<u16>, SecurityError> {
    let mut buffer: Vec<u16> = value.encode_wide().collect();
    if buffer.contains(&0) {
        return Err(SecurityError::InvalidName);
    }
    buffer.push(0);
    Ok(buffer)
}

fn last_error() -> SecurityError {
    // SAFETY: `GetLastError` no tiene precondiciones y no toca el estado de Rust.
    match unsafe { GetLastError() } {
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => SecurityError::Missing,
        _ => SecurityError::Native,
    }
}

fn trim_trailing_nul(units: &[u16]) -> &[u16] {
    match units.split_last() {
        Some((0, head)) => head,
        _ => units,
    }
}

fn utf16_to_string(units: &[u16]) -> String {
    String::from_utf16_lossy(trim_trailing_nul(units))
}

fn sid_to_string(sid: PSID) -> Result<String, SecurityError> {
    if sid.is_null() {
        return Err(SecurityError::Native);
    }
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: `sid` pertenece a un descriptor vivo en el llamador.
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(SecurityError::Native);
    }
    let _memory = LocalMemory(text.cast());
    if text.is_null() {
        return Err(SecurityError::Native);
    }
    let mut length = 0_usize;
    // SAFETY: la API devuelve una cadena UTF-16 terminada en cero propiedad de
    // `_memory`, que vive hasta el final de la función.
    unsafe {
        while *text.add(length) != 0 {
            length += 1;
        }
        Ok(utf16_to_string(std::slice::from_raw_parts(text, length)))
    }
}

/// SID del usuario efectivo del proceso en forma `S-1-...`.
pub(super) fn current_user_sid() -> Result<String, SecurityError> {
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: se pide `TOKEN_QUERY` sobre el pseudo-handle del propio proceso y
    // se escribe un único handle en `token`.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(SecurityError::Native);
    }
    let token = OwnedHandle::adopt(token)?;

    let mut needed = 0_u32;
    // SAFETY: búfer nulo con longitud cero es la forma documentada de consultar
    // el tamaño requerido; la llamada no escribe nada.
    unsafe {
        GetTokenInformation(token.raw(), TokenUser, ptr::null_mut(), 0, &mut needed);
    }
    if needed == 0 {
        return Err(SecurityError::Native);
    }
    // `TOKEN_USER` contiene punteros y exige alineación de palabra; se reserva
    // como `u64` para no depender de la alineación de `Vec<u8>`.
    let words = usize::try_from(needed)
        .map_err(|_| SecurityError::Native)?
        .div_ceil(size_of::<u64>());
    let mut buffer = vec![0_u64; words];
    // SAFETY: el búfer tiene al menos `needed` bytes y se declara esa longitud.
    if unsafe {
        GetTokenInformation(
            token.raw(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return Err(SecurityError::Native);
    }
    // SAFETY: la llamada anterior rellenó un `TOKEN_USER` alineado en `buffer`,
    // que sigue vivo durante toda la expresión.
    let user: &TOKEN_USER = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let sid = user.User.Sid;
    sid_to_string(sid)
}

/// Propietario y representación SDDL del descriptor vigente del handle.
pub(super) fn owner_and_sddl(handle: HANDLE) -> Result<(String, String), SecurityError> {
    let mut owner: PSID = ptr::null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: se piden propietario y DACL de un handle abierto con
    // `READ_CONTROL`; todas las salidas son punteros a locales vivos.
    let status = unsafe {
        GetSecurityInfo(
            handle,
            SE_FILE_OBJECT,
            DESCRIPTOR_PARTS,
            &mut owner,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(SecurityError::Native);
    }
    let _memory = LocalMemory(descriptor.cast());
    let owner_sid = sid_to_string(owner)?;

    let mut text: *mut u16 = ptr::null_mut();
    let mut length = 0_u32;
    // SAFETY: `descriptor` sigue vivo en `_memory` y se solicita la misma
    // información que se leyó.
    if unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DESCRIPTOR_PARTS,
            &mut text,
            &mut length,
        )
    } == 0
    {
        return Err(SecurityError::Native);
    }
    let _text_memory = LocalMemory(text.cast());
    let units = usize::try_from(length).map_err(|_| SecurityError::Native)?;
    if units == 0 || text.is_null() {
        return Err(SecurityError::Native);
    }
    // SAFETY: `length` cuenta unidades UTF-16 incluido el terminador y el búfer
    // pertenece a `_text_memory`.
    let sddl = unsafe { utf16_to_string(std::slice::from_raw_parts(text, units)) };
    Ok((owner_sid, sddl))
}

fn security_descriptor_from(sddl: &str) -> Result<LocalMemory, SecurityError> {
    let text = wide(OsStr::new(sddl))?;
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let mut size = 0_u32;
    // SAFETY: la cadena está terminada en cero y las salidas son locales.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            &mut size,
        )
    } == 0
    {
        return Err(SecurityError::Native);
    }
    if descriptor.is_null() {
        return Err(SecurityError::Native);
    }
    Ok(LocalMemory(descriptor.cast()))
}

fn security_attributes(descriptor: &LocalMemory) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    }
}

/// Abre un handle del directorio sin seguir puntos de reanálisis.
pub(super) fn open_directory(path: &Path) -> Result<OwnedHandle, SecurityError> {
    let name = wide(path.as_os_str())?;
    // SAFETY: ruta terminada en cero, sin atributos de creación y con banderas
    // que piden el propio objeto, no su destino.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    OwnedHandle::adopt(handle)
}

/// Abre un archivo existente en solo lectura sin seguir puntos de reanálisis.
///
/// `FILE_FLAG_BACKUP_SEMANTICS` permite abrir también un directorio, de modo que
/// la comprobación de tipo puede clasificarlo como tal en lugar de depender de
/// un código de error ambiguo.
pub(super) fn open_regular_file(path: &Path) -> Result<OwnedHandle, SecurityError> {
    let name = wide(path.as_os_str())?;
    // SAFETY: mismas garantías que [`open_directory`], sobre el objeto final.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            FILE_READ_DATA | FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(last_error());
    }
    OwnedHandle::adopt(handle)
}

/// Crea un directorio con la DACL indicada, sin heredarla del padre.
pub(super) fn create_directory_with(path: &Path, sddl: &str) -> Result<(), SecurityError> {
    let descriptor = security_descriptor_from(sddl)?;
    let attributes = security_attributes(&descriptor);
    let name = wide(path.as_os_str())?;
    // SAFETY: la ruta y los atributos están vivos durante la llamada y el
    // descriptor incluye una DACL protegida completa.
    if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
        return Err(match unsafe { GetLastError() } {
            ERROR_ALREADY_EXISTS | ERROR_FILE_EXISTS => SecurityError::AlreadyExists,
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => SecurityError::Missing,
            _ => SecurityError::Native,
        });
    }
    Ok(())
}

/// Crea un archivo nuevo con la DACL indicada, en una sola operación.
pub(super) fn create_file_with(path: &Path, sddl: &str) -> Result<OwnedHandle, SecurityError> {
    let descriptor = security_descriptor_from(sddl)?;
    let attributes = security_attributes(&descriptor);
    let name = wide(path.as_os_str())?;
    // SAFETY: `CREATE_NEW` garantiza exclusividad y el descriptor se aplica de
    // forma atómica al objeto recién creado.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            FILE_WRITE_DATA | FILE_READ_ATTRIBUTES | READ_CONTROL,
            FILE_SHARE_READ,
            &attributes,
            CREATE_NEW,
            FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(match unsafe { GetLastError() } {
            ERROR_FILE_EXISTS | ERROR_ALREADY_EXISTS => SecurityError::AlreadyExists,
            ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND => SecurityError::Missing,
            _ => SecurityError::Native,
        });
    }
    OwnedHandle::adopt(handle)
}

/// Banderas de atributo del objeto real del handle.
pub(super) fn attributes(handle: HANDLE) -> Result<Attributes, SecurityError> {
    let mut info = FILE_ATTRIBUTE_TAG_INFO {
        FileAttributes: 0,
        ReparseTag: 0,
    };
    // SAFETY: se pide `FileAttributeTagInfo`, cuyo tamaño coincide con el tipo.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            handle,
            FileAttributeTagInfo,
            ptr::addr_of_mut!(info).cast(),
            u32::try_from(size_of::<FILE_ATTRIBUTE_TAG_INFO>()).unwrap_or(0),
        )
    };
    if ok == 0 {
        return Err(SecurityError::Native);
    }
    Ok(Attributes {
        flags: info.FileAttributes,
    })
}

/// Ruta final canónica del objeto abierto, tal como la resuelve el sistema.
pub(super) fn final_path(handle: HANDLE) -> Result<String, SecurityError> {
    let mut buffer = vec![0_u16; 512];
    let capacity = u32::try_from(buffer.len()).map_err(|_| SecurityError::Native)?;
    // SAFETY: búfer válido de la longitud declarada; sin banderas adicionales.
    let mut length = usize::try_from(unsafe {
        GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), capacity, 0)
    })
    .map_err(|_| SecurityError::Native)?;
    if length >= buffer.len() {
        buffer.resize(length + 1, 0);
        let capacity = u32::try_from(buffer.len()).map_err(|_| SecurityError::Native)?;
        // SAFETY: el búfer se redimensionó por encima de la longitud pedida.
        let retried =
            unsafe { GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), capacity, 0) };
        length = usize::try_from(retried).map_err(|_| SecurityError::Native)?;
    }
    if length == 0 || length > buffer.len() {
        return Err(SecurityError::Native);
    }
    Ok(String::from_utf16_lossy(&buffer[..length]))
}

/// Lee todo el contenido del handle desde su posición actual.
pub(super) fn read_all(handle: HANDLE) -> Result<Vec<u8>, SecurityError> {
    // `ManuallyDrop` evita que `File` cierre un handle que sigue siendo
    // propiedad de [`OwnedHandle`].
    let mut file =
        ManuallyDrop::new(unsafe { std::fs::File::from_raw_handle(handle as RawHandle) });
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)
        .map_err(|_| SecurityError::Native)?;
    Ok(contents)
}

/// Escribe todo el contenido y lo sincroniza con el disco.
pub(super) fn write_all(handle: HANDLE, contents: &[u8]) -> Result<(), SecurityError> {
    let mut file =
        ManuallyDrop::new(unsafe { std::fs::File::from_raw_handle(handle as RawHandle) });
    file.write_all(contents)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|_| SecurityError::Native)
}
