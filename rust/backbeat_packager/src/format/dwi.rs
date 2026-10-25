//! Package `.dwi` files into [`BackbeatFile`]s.

use std::io;
use std::path::Path;

use rg_formats::sm_msd;

use crate::deps::ensure_asset;
use crate::seen_cache::SeenCache;
use backbeat_core::{BackbeatFile, BackbeatFileError};

#[derive(Debug, thiserror::Error)]
pub enum DwiPackageError {
	#[error(transparent)]
	FromFile(#[from] crate::error::FromFileError),

	#[error("DWI asset path in #{tag} is not valid UTF-8: {source}")]
	InvalidAssetPathUtf8 {
		tag: &'static str,
		#[source]
		source: std::str::Utf8Error,
	},

	#[error("could not build BackbeatFile: {0}")]
	Build(#[from] BackbeatFileError),

	#[error("could not resolve DWI assets: {0}")]
	Io(#[from] io::Error),
}

pub(crate) fn resolve_assets(
	bb: &mut BackbeatFile,
	dwi_path: &Path,
	cache: &SeenCache,
) -> Result<(), DwiPackageError> {
	let chart_dir = dwi_path.parent().unwrap_or(Path::new("."));
	let bytes = bb.chart.decompress();
	let msd = sm_msd::from_bytes(&bytes);
	for (real_tag, fallback_tag) in [
		("REALMUSIC", "FILE"),
		("REALBANNER", "BANNER"),
		("REALBACKGROUND", "BACKGROUND"),
		("REALJACKET", "JACKET"),
		("REALCDTITLE", "CDTITLE"),
	] {
		let selected = get_tag_value(&msd.elements, real_tag)
			.map(|path| (real_tag, path))
			.or_else(|| {
				get_tag_value(&msd.elements, fallback_tag).map(|path| (fallback_tag, path))
			});
		let Some((tag, path)) = selected else {
			continue;
		};
		let path = std::str::from_utf8(path)
			.map_err(|source| DwiPackageError::InvalidAssetPathUtf8 { tag, source })?;
		ensure_asset(bb, chart_dir, cache, path, &[])?;
	}
	Ok(())
}

fn get_tag_value<'a>(
	elements: &'a [rg_formats::sm_msd::MsdElement],
	tag: &str,
) -> Option<&'a [u8]> {
	elements
		.iter()
		.find(|element| element.tag.as_ref() == tag.as_bytes())
		.and_then(|element| element.values.first())
		.map(Box::as_ref)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn accepts_non_utf8_outside_selected_asset_paths() {
		let raw = b"#TITLE:\x80;\n#FILE:\x80.ogg;\n#REALMUSIC:song.ogg;\n";
		let chart_path = Path::new("test.dwi");
		let cache = SeenCache::new();
		let mut bb = crate::base::from_bytes(raw, chart_path, &cache).unwrap();

		resolve_assets(&mut bb, chart_path, &cache).unwrap();

		assert_eq!(
			cache.missing_asset_paths(),
			vec![backbeat_core::AssetPath::from_path("song.ogg").unwrap()]
		);
	}

	#[test]
	fn rejects_non_utf8_real_asset_paths() {
		let raw = b"#FILE:song.ogg;\n#REALMUSIC:\x80.ogg;\n";
		let chart_path = Path::new("test.dwi");
		let cache = SeenCache::new();
		let mut bb = crate::base::from_bytes(raw, chart_path, &cache).unwrap();

		let error = resolve_assets(&mut bb, chart_path, &cache).unwrap_err();

		assert!(matches!(
			error,
			DwiPackageError::InvalidAssetPathUtf8 {
				tag: "REALMUSIC",
				..
			}
		));
	}
}
