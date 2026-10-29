//! Políticas Argon2id explícitas y límites de entrada.

use argon2::Params;
use argon2::password_hash::ParamsString;

use crate::CredentialError;

/// Longitud máxima del string PHC aceptado.
pub const MAX_PHC_BYTES: usize = 256;
/// Longitud máxima de contraseña o PIN en bytes UTF-8.
pub const MAX_PASSWORD_BYTES: usize = 1024;
/// Límite duro de BCrypt: 72 bytes, sin truncado.
pub const MAX_BCRYPT_PASSWORD_BYTES: usize = 72;
/// Coste BCrypt máximo admitido en laboratorio.
pub const MAX_BCRYPT_COST: u32 = 12;
/// Memoria Argon2id máxima admitida.
pub const MAX_ARGON2_MEMORY_KIB: u32 = 32_768;
/// Iteraciones Argon2id máximas admitidas.
pub const MAX_ARGON2_ITERATIONS: u32 = 3;
/// Carriles Argon2id máximos admitidos.
pub const MAX_ARGON2_LANES: u32 = 2;
/// Longitud de salida Argon2id exigida por el contrato de laboratorio.
pub const ARGON2_OUTPUT_BYTES: usize = 32;
/// Longitud de sal solicitada al generar material nuevo.
pub const ARGON2_SALT_BYTES: usize = 16;

/// Política Argon2id declarada explícitamente.
///
/// `LAB_V0` existe solo para reconocer material antiguo y migrarlo; `LAB_V1` es
/// el destino de rehash. Ninguna de las dos está aprobada para producción.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Argon2Policy {
    id: &'static str,
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
}

impl Argon2Policy {
    /// Política antigua reconocida solo para verificar y migrar.
    pub const LAB_V0: Self = Self {
        id: "lab-argon2-v0",
        memory_kib: 8_192,
        iterations: 1,
        lanes: 1,
    };
    /// Destino de rehash **experimental**.
    pub const LAB_V1: Self = Self {
        id: "lab-argon2-v1",
        memory_kib: 19_456,
        iterations: 2,
        lanes: 1,
    };

    /// Identificador estable usado como `policy_id`.
    pub fn id(self) -> &'static str {
        self.id
    }
    /// Memoria en KiB.
    pub fn memory_kib(self) -> u32 {
        self.memory_kib
    }
    /// Iteraciones (`t`).
    pub fn iterations(self) -> u32 {
        self.iterations
    }
    /// Carriles (`p`).
    pub fn lanes(self) -> u32 {
        self.lanes
    }

    /// Resuelve una política registrada por identificador.
    pub fn from_id(id: &str) -> Option<Self> {
        [Self::LAB_V0, Self::LAB_V1]
            .into_iter()
            .find(|policy| policy.id == id)
    }

    /// Construye parámetros Argon2id con salida fija.
    pub(crate) fn params(self) -> Result<Params, CredentialError> {
        if self.memory_kib > MAX_ARGON2_MEMORY_KIB
            || self.iterations > MAX_ARGON2_ITERATIONS
            || self.lanes > MAX_ARGON2_LANES
        {
            return Err(CredentialError::PolicyRejected);
        }
        Params::new(
            self.memory_kib,
            self.iterations,
            self.lanes,
            Some(ARGON2_OUTPUT_BYTES),
        )
        .map_err(|_| CredentialError::PolicyRejected)
    }

    /// Comprueba coincidencia **exacta** con los parámetros de un PHC.
    ///
    /// Estar por debajo del techo no basta: el conjunto debe ser exactamente
    /// `m`, `t` y `p` de una política registrada.
    pub(crate) fn matches(self, params: &ParamsString) -> bool {
        let mut seen = 0_u8;
        for (key, value) in params.iter() {
            let expected = match key.as_str() {
                "m" => self.memory_kib,
                "t" => self.iterations,
                "p" => self.lanes,
                _ => return false,
            };
            if value.decimal().ok() != Some(expected) {
                return false;
            }
            seen += 1;
        }
        seen == 3
    }
}
