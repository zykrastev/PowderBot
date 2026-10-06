use crate::profile::{Profile, ProfileError, valid_name, validate_storage_name};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_FILE_BYTES: usize = 4096;
pub const MAX_PROFILES: usize = 64;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Invalid(ProfileError),
    Corrupt(String),
    TooLarge,
    Capacity,
    AlreadyExists,
    NotFound,
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Storage I/O: {e}"),
            Self::Invalid(e) => write!(f, "{e}"),
            Self::Corrupt(e) => write!(f, "Invalid stored record: {e}"),
            Self::TooLarge => write!(f, "File exceeds {MAX_FILE_BYTES} bytes"),
            Self::Capacity => write!(f, "Profile limit is {MAX_PROFILES}"),
            Self::AlreadyExists => f.write_str("Profile already exists"),
            Self::NotFound => f.write_str("Profile does not exist"),
        }
    }
}
impl Error for StoreError {}
impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<ProfileError> for StoreError {
    fn from(e: ProfileError) -> Self {
        Self::Invalid(e)
    }
}

#[derive(Debug)]
pub struct StoredProfile {
    pub storage_name: String,
    pub profile: Profile,
}
#[derive(Debug, Default)]
pub struct ProfileListing {
    pub profiles: Vec<StoredProfile>,
    pub diagnostics: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Selection {
    storage_name: Option<String>,
}

pub struct ProfileStore {
    root: PathBuf,
}
impl ProfileStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_owned();
        if !fs::metadata(&root)?.is_dir() {
            return Err(StoreError::Corrupt("Store root is not a directory".into()));
        }
        Ok(Self { root })
    }

    pub fn read(&mut self, name: &str) -> Result<Profile, StoreError> {
        validate_storage_name(name)?;
        let bytes = self
            .recover(name, validate_profile)?
            .ok_or(StoreError::NotFound)?;
        Ok(Profile::from_json(&bytes)?)
    }

    pub fn create(&mut self, name: &str, profile: &Profile) -> Result<(), StoreError> {
        validate_storage_name(name)?;
        let bytes = profile.to_json()?;
        // Corrupt slots are occupied too: creation must never overwrite one.
        if self.slot_exists(name)? {
            return Err(StoreError::AlreadyExists);
        }
        let names = self.names()?;
        if names.len() >= MAX_PROFILES {
            return Err(StoreError::Capacity);
        }
        self.replace(name, &bytes, validate_profile)
    }

    pub fn update(&mut self, name: &str, profile: &Profile) -> Result<(), StoreError> {
        self.read(name)?;
        self.replace(name, &profile.to_json()?, validate_profile)
    }

    pub fn list(&mut self) -> Result<ProfileListing, StoreError> {
        let mut listing = ProfileListing::default();
        let names = self.names()?;
        if names.len() > MAX_PROFILES {
            listing.diagnostics.push(format!(
                "Profile listing truncated to {MAX_PROFILES}; delete excess profiles by name"
            ));
        }
        for name in names.into_iter().take(MAX_PROFILES) {
            match self.read(&name) {
                Ok(profile) => listing.profiles.push(StoredProfile {
                    storage_name: name,
                    profile,
                }),
                Err(StoreError::Io(error)) => return Err(StoreError::Io(error)),
                Err(error) => listing.diagnostics.push(format!("{name}: {error}")),
            }
        }
        Ok(listing)
    }

    pub fn active(&mut self) -> Result<Option<StoredProfile>, StoreError> {
        let Some(bytes) = self.recover("active_profile", validate_selection)? else {
            return Ok(None);
        };
        let selection: Selection = serde_json::from_slice(&bytes).map_err(corrupt)?;
        match selection.storage_name {
            Some(name) => Ok(Some(StoredProfile {
                profile: self.read(&name)?,
                storage_name: name,
            })),
            None => Ok(None),
        }
    }

    pub fn select(&mut self, name: Option<&str>) -> Result<(), StoreError> {
        if let Some(name) = name {
            self.read(name)?;
        }
        let bytes = serde_json::to_vec(&Selection {
            storage_name: name.map(str::to_owned),
        })
        .map_err(corrupt)?;
        // Explicit selection can repair malformed metadata, but never hide I/O errors.
        match self.recover("active_profile", validate_selection) {
            Err(StoreError::Io(e)) => return Err(StoreError::Io(e)),
            Err(_) => {
                self.remove_file("active_profile", "tmp")?;
                self.remove_file("active_profile", "bak")?;
                self.remove_file("active_profile", "json")?;
            }
            Ok(_) => {}
        }
        self.replace("active_profile", &bytes, validate_selection)
    }

    pub fn remove(&mut self, name: &str) -> Result<(), StoreError> {
        validate_storage_name(name)?;
        if !self.slot_exists(name)? {
            return Err(StoreError::NotFound);
        }
        match self.active() {
            Ok(Some(active)) if active.storage_name == name => self.select(None)?,
            Err(StoreError::Io(e)) => return Err(StoreError::Io(e)),
            Err(_) => self.select(None)?, // explicitly clear broken selection before deletion
            _ => {}
        }
        // Remove recovery files first so a completed deletion cannot resurrect a profile.
        self.remove_file(name, "tmp")?;
        self.remove_file(name, "bak")?;
        self.remove_file(name, "json")
    }

    fn names(&self) -> Result<BTreeSet<String>, StoreError> {
        let mut names = BTreeSet::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let filename = entry.file_name();
            let Some(filename) = filename.to_str() else {
                continue;
            };
            let Some((name, suffix)) = filename.rsplit_once('.') else {
                continue;
            };
            if matches!(suffix, "json" | "bak") && validate_storage_name(name).is_ok() {
                names.insert(name.to_owned());
                if names.len() > MAX_PROFILES {
                    break;
                }
            }
        }
        Ok(names)
    }

    fn path(&self, name: &str, suffix: &str) -> PathBuf {
        self.root.join(format!("{name}.{suffix}"))
    }
    fn slot_exists(&self, name: &str) -> Result<bool, StoreError> {
        for suffix in ["json", "bak"] {
            match fs::metadata(self.path(name, suffix)) {
                Ok(_) => return Ok(true),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(false)
    }
    fn remove_file(&self, name: &str, suffix: &str) -> Result<(), StoreError> {
        match fs::remove_file(self.path(name, suffix)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn load(
        &self,
        name: &str,
        suffix: &str,
        validate: fn(&[u8]) -> Result<(), StoreError>,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let file = match File::open(self.path(name, suffix)) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Vec::new();
        file.take((MAX_FILE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(StoreError::TooLarge);
        }
        validate(&bytes)?;
        Ok(Some(bytes))
    }

    fn recover(
        &self,
        name: &str,
        validate: fn(&[u8]) -> Result<(), StoreError>,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        let primary = self.load(name, "json", validate);
        match &primary {
            Ok(Some(_)) => return primary,
            Err(StoreError::Io(_)) => return primary,
            _ => {}
        }
        if let Some(bytes) = self.load(name, "bak", validate)? {
            self.remove_file(name, "json")?;
            fs::rename(self.path(name, "bak"), self.path(name, "json"))?;
            Ok(Some(bytes))
        } else {
            primary
        }
    }

    fn replace(
        &self,
        name: &str,
        bytes: &[u8],
        validate: fn(&[u8]) -> Result<(), StoreError>,
    ) -> Result<(), StoreError> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(StoreError::TooLarge);
        }
        validate(bytes)?;
        let previous = self.recover(name, validate)?;
        self.remove_file(name, "tmp")?;
        let mut file = File::create(self.path(name, "tmp"))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        let written = self
            .load(name, "tmp", validate)?
            .ok_or(StoreError::NotFound)?;
        if written != bytes {
            return Err(StoreError::Corrupt("Temporary write differs".into()));
        }
        self.remove_file(name, "bak")?;
        if previous.is_some() {
            fs::rename(self.path(name, "json"), self.path(name, "bak"))?;
        }
        fs::rename(self.path(name, "tmp"), self.path(name, "json"))?;
        let written = self
            .load(name, "json", validate)?
            .ok_or(StoreError::NotFound)?;
        if written != bytes {
            return Err(StoreError::Corrupt("Replacement differs".into()));
        }
        self.remove_file(name, "bak")
    }
}
fn corrupt(e: serde_json::Error) -> StoreError {
    StoreError::Corrupt(e.to_string())
}
fn validate_profile(bytes: &[u8]) -> Result<(), StoreError> {
    Profile::from_json(bytes)?;
    Ok(())
}
fn validate_selection(bytes: &[u8]) -> Result<(), StoreError> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(corrupt)?;
    // Missing is invalid; null is the explicit cleared-selection record.
    if value.get("storageName").is_none() {
        return Err(StoreError::Corrupt("Missing storageName".into()));
    }
    let selection: Selection = serde_json::from_value(value).map_err(corrupt)?;
    if let Some(name) = selection.storage_name {
        if !valid_name(&name, 26) {
            return Err(StoreError::Corrupt("Invalid selected name".into()));
        }
        validate_storage_name(&name)?;
    }
    Ok(())
}
