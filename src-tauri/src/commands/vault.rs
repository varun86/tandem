// ============================================================================
// Vault Commands (PIN-based encryption)
// ============================================================================

/// Get the current vault status
#[tauri::command]
pub fn get_vault_status(vault_state: State<'_, VaultState>) -> VaultStatus {
    vault_state.get_status()
}

async fn initialize_keystore_after_unlock(app: AppHandle, master_key: Vec<u8>) {
    let app_clone = app.clone();
    let init_result = tauri::async_runtime::spawn_blocking(move || {
        crate::init_keystore_and_keys(&app_clone, &master_key);
    })
    .await;

    match init_result {
        Ok(()) => tracing::info!("Keystore initialization complete"),
        Err(err) => tracing::error!("Keystore initialization task failed: {}", err),
    }
}

fn spawn_sidecar_start_in_background(app: AppHandle) {
    tauri::async_runtime::spawn_blocking(move || {
        let result = match app.try_state::<AppState>() {
            Some(state) => tauri::async_runtime::block_on(start_sidecar_inner(&app, state.inner())),
            None => Err(TandemError::Sidecar(
                "App state unavailable for background sidecar start".to_string(),
            )),
        };

        if let Err(err) = result {
            tracing::warn!(
                "Vault unlocked but failed to auto-start tandem-engine sidecar: {}",
                err
            );
        }
    });
}

/// Create a new vault with a PIN
#[tauri::command]
pub async fn create_vault(
    app: AppHandle,
    vault_state: State<'_, VaultState>,
    state: State<'_, AppState>,
    pin: String,
) -> Result<()> {
    // Validate PIN
    vault::validate_pin(&pin)?;

    // Check if vault already exists
    if vault::vault_exists(&vault_state.app_data_dir) {
        return Err(TandemError::Vault("Vault already exists".to_string()));
    }

    // Delete any existing legacy Stronghold snapshot (from previous installations)
    let stronghold_path = vault_state.app_data_dir.join("tandem.stronghold");
    if stronghold_path.exists() {
        tracing::warn!("Deleting old Stronghold snapshot: {:?}", stronghold_path);
        std::fs::remove_file(&stronghold_path).ok();
    }

    // Create encrypted vault key
    let (encrypted_key, master_key) = EncryptedVaultKey::create(&pin)?;

    // Save to file
    let vault_key_path = vault::get_vault_key_path(&vault_state.app_data_dir);
    encrypted_key.save(&vault_key_path)?;

    tracing::info!("Created new vault at {:?}", vault_key_path);

    // Store master key and mark as unlocked
    vault_state.set_master_key(master_key.clone());

    // Ensure keystore is initialized before sidecar startup so provider auth is available immediately.
    initialize_keystore_after_unlock(app.clone(), master_key.clone()).await;

    // Start the sidecar as part of lock-screen unlock/create flow.
    // Startup failures must not block vault creation.
    let _ = state;
    spawn_sidecar_start_in_background(app.clone());

    Ok(())
}

// ============================================================================
