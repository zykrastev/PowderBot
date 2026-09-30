//! Owns the flash mount. Dropping this object unmounts LittleFS.
use anyhow::Context;
use esp_idf_svc::{fs::littlefs::Littlefs, io::vfs::MountedLittlefs};
use powderbot_core::profile_store::ProfileStore;
use std::sync::{Arc, Mutex};

pub struct Storage {
    pub profiles: Arc<Mutex<ProfileStore>>,
    _mount: MountedLittlefs<Littlefs<()>>,
}

impl Storage {
    pub fn mount() -> anyhow::Result<Self> {
        let mount = match Self::mount_partition() {
            Ok(mount) => mount,
            Err(error) => {
                #[cfg(not(feature = "storage-init"))]
                return Err(error)
                    .context("LittleFS unavailable; normal firmware never formats storage");
                #[cfg(feature = "storage-init")]
                {
                    log::warn!("LittleFS mount failed ({error}); storage-init will ERASE the storage partition");
                    // SAFETY: the failed mount released its owner; no other code uses this partition.
                    let mut filesystem = unsafe { Littlefs::<()>::new_partition("storage")? };
                    filesystem.format().context("Formatting LittleFS")?;
                    drop(filesystem);
                    Self::mount_partition().context("Mount after formatting")?
                }
            }
        };
        let info = mount.info()?;
        log::info!(
            "LittleFS mounted: {} / {} bytes used",
            info.used_bytes,
            info.total_bytes
        );
        let mut profiles = ProfileStore::open("/storage")?;
        let listing = profiles.list()?;
        log::info!("Storage: {} valid profiles", listing.profiles.len());
        for diagnostic in listing.diagnostics {
            log::warn!("Storage: {diagnostic}");
        }
        match profiles.active() {
            Ok(Some(active)) => log::info!("Active profile: {}", active.storage_name),
            Ok(None) => log::info!("No active profile selected"),
            Err(error) => log::warn!("No active profile: {error}"),
        }
        Ok(Self {
            profiles: Arc::new(Mutex::new(profiles)),
            _mount: mount,
        })
    }

    fn mount_partition() -> Result<MountedLittlefs<Littlefs<()>>, esp_idf_svc::sys::EspError> {
        // SAFETY: main creates one Storage owner; the table defines this partition,
        // and no other module mounts, formats or accesses it through raw flash APIs.
        let filesystem = unsafe { Littlefs::<()>::new_partition("storage")? };
        MountedLittlefs::mount(filesystem, "/storage")
    }

    #[cfg(feature = "storage-test")]
    pub fn probe(&self) -> anyhow::Result<()> {
        use std::{
            fs::OpenOptions,
            io::{Read, Write},
        };
        const PATH: &str = "/storage/storage_probe.json";
        const RECORD: &[u8] = b"{\"version\":1,\"probe\":\"PowderBot persistence\"}";
        match OpenOptions::new().write(true).create_new(true).open(PATH) {
            Ok(mut file) => {
                file.write_all(RECORD)?;
                file.sync_all()?;
                drop(file);
                anyhow::ensure!(std::fs::read(PATH)? == RECORD, "Probe readback differs");
                log::info!("Storage probe: written and verified; reboot to check persistence");
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let mut bytes = Vec::new();
                std::fs::File::open(PATH)?
                    .take((RECORD.len() + 1) as u64)
                    .read_to_end(&mut bytes)?;
                anyhow::ensure!(
                    bytes == RECORD,
                    "Unexpected probe contents; leaving record untouched"
                );
                log::info!("Storage probe: existing record verified (persistence OK)");
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }
}
