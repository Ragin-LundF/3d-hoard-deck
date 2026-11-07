use std::sync::Arc;

use hoarddeck_domain::{Slicer, SlicerList};

use crate::AppError;
use crate::ports::{SlicerDetector, SlicerStore};

/// Reads, saves and fills the slicer list. On first use the installed slicers are detected.
pub struct ManageSlicers {
    pub store: Arc<dyn SlicerStore>,
    pub detector: Arc<dyn SlicerDetector>,
}

impl ManageSlicers {
    /// # Errors
    /// Fails when the index cannot be read or written.
    pub fn list(&self) -> Result<SlicerList, AppError> {
        if let Some(list) = self.store.load_slicers()? {
            return Ok(list);
        }
        let list = SlicerList { slicers: self.detector.installed() };
        self.store.save_slicers(&list)?;
        Ok(list)
    }

    /// Saves the whole list as edited by the user; the first entry is the default.
    /// # Errors
    /// Fails for an invalid entry or when the index cannot be written.
    pub fn save(&self, slicers: Vec<Slicer>) -> Result<SlicerList, AppError> {
        let slicers = slicers
            .into_iter()
            .map(|slicer| Slicer::new(&slicer.name, &slicer.program, slicer.args, &slicer.extensions))
            .collect::<Result<Vec<_>, _>>()?;
        let list = SlicerList { slicers };
        self.store.save_slicers(&list)?;
        Ok(list)
    }

    /// Adds installed slicers that are not in the list yet.
    /// # Errors
    /// Fails when the index cannot be read or written.
    pub fn detect(&self) -> Result<SlicerList, AppError> {
        let mut list = self.list()?;
        for found in self.detector.installed() {
            if !list.slicers.iter().any(|known| known.program == found.program) {
                list.slicers.push(found);
            }
        }
        self.store.save_slicers(&list)?;
        Ok(list)
    }
}
