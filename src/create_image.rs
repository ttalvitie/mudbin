use crate::errors::*;

use std::path::Path;

use log::warn;

pub fn create_image(_output_path: &Path) -> Result<()> {
    warn!("Image creation not implemented");
    Ok(())
}
