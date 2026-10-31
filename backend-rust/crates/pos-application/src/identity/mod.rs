//! Autenticación Local: login, identidad actual, sucursal y sesiones.
//!
//! El caso de uso no conoce SQLite ni criptografía: depende de puertos que el
//! adaptador implementa sobre el esquema productivo (`0003`/`0004`) y de
//! `pos-credentials` para Argon2id.
//!
//! # Reglas
//!
//! - La organización y los permisos proceden de la base, nunca del consumidor.
//! - El handle de sesión se devuelve una sola vez; solo su digest se persiste.
//! - Un rechazo es siempre el mismo error genérico.

mod dto;
mod ports;
mod use_cases;

pub use dto::*;
pub use ports::*;
pub use use_cases::*;
