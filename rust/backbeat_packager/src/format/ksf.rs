//! It's a format nobody has ever heard of; for "Kick it up".
//!
//! I've encountered some of these files in the wild and need to parse them,
//! so, I guess we shall do that.
use std::path::Path;

use backbeat_core::{BackbeatFile, BackbeatFileError};

use crate::deps::ensure_asset;
use crate::seen_cache::SeenCache;

#[derive(Debug, thiserror::Error)]
pub enum KsfPackageError {
	#[error(transparent)]
	FromFile(#[from] crate::error::FromFileError),

	#[error("could not parse KSF chart: {0}")]
	Parse(#[from] rg_formats::ksf::LoadError),

	#[error("could not build BackbeatFile: {0}")]
	Build(#[from] BackbeatFileError),
}

pub(crate) fn resolve_assets(
	bb: &mut BackbeatFile,
	ksf_path: &Path,
	cache: &SeenCache,
) -> Result<(), KsfPackageError> {
	let chart_dir = ksf_path.parent().unwrap_or(Path::new("."));
	let bytes = bb.chart.decompress();
	let chart = rg_formats::ksf::from_bytes(&bytes, ksf_path)?;

	for path in [
		chart.music.as_deref(),
		chart.banner.as_deref(),
		chart.background.as_deref(),
		chart.preview_music.as_deref(),
	]
	.into_iter()
	.flatten()
	{
		ensure_asset(bb, chart_dir, cache, path, &[])?;
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use std::fs;

	#[test]
	fn resolves_explicit_ksf_assets() {
		let temp = tempfile::TempDir::new().unwrap();
		let chart_path = temp.path().join("normal_1.ksf");
		fs::write(temp.path().join("song.ogg"), b"audio").unwrap();
		fs::write(temp.path().join("disc.bmp"), b"banner").unwrap();
		fs::write(
			&chart_path,
			b"#SONGFILE:song.ogg;#DISCFILE:disc.bmp;#STEP:\n0000000000000\n2222222222222",
		)
		.unwrap();

		let packages = crate::package(&chart_path).unwrap();
		assert_eq!(packages.len(), 1);
		assert!(
			packages[0]
				.assets
				.contains_key(&backbeat_core::AssetPath::from_path("song.ogg").unwrap())
		);
		assert!(
			packages[0]
				.assets
				.contains_key(&backbeat_core::AssetPath::from_path("disc.bmp").unwrap())
		);
	}
}
