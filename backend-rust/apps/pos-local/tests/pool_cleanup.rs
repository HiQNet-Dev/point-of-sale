//! Regresión del checkpoint y cierre con un pool SQLite real, no futuros simulados.

use pos_local::shutdown::checkpoint_and_close;
use pos_storage_sqlite::{checkpoint, open};
use std::{error::Error, time::Duration};

#[tokio::test]
async fn real_pool_stays_open_until_checkpoint_finishes() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let pool = open(
        &temp.path().join("local.sqlite"),
        Duration::from_millis(100),
    )
    .await?;
    let operation = async {
        sqlx::query("CREATE TABLE checkpoint_probe (id INTEGER PRIMARY KEY) STRICT")
            .execute(&pool)
            .await?;
        sqlx::query("INSERT INTO checkpoint_probe (id) VALUES (1)")
            .execute(&pool)
            .await?;
        checkpoint_and_close(checkpoint(&pool), async { pool.close().await }).await?;
        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    let closed_by_helper = pool.is_closed();
    // También liberar el pool si falla la preparación o el checkpoint de la prueba.
    pool.close().await;
    operation?;
    assert!(closed_by_helper);
    Ok(())
}
