//! Package `.sm` files into a [`BackbeatFile`].

use std::io;
use std::path::Path;

use rg_formats::sm_msd::{self, MsdElement};

use crate::seen_cache::SeenCache;
use backbeat_core::{BackbeatFile, BackbeatFileError};

use crate::deps::ensure_asset;

/// Errors that can occur when packaging an SM file into a [`BackbeatFile`].
#[derive(Debug, thiserror::Error)]
pub enum SmPackageError {
	#[error(transparent)]
	FromFile(#[from] crate::error::FromFileError),

	#[error("SM asset path in #{tag} is not valid UTF-8: {source}")]
	InvalidAssetPathUtf8 {
		tag: &'static str,
		#[source]
		source: std::str::Utf8Error,
	},

	#[error("could not build BackbeatFile: {0}")]
	Build(#[from] BackbeatFileError),

	#[error("could not resolve SM assets: {0}")]
	Io(#[from] io::Error),
}

pub(crate) fn resolve_assets(
	bb: &mut BackbeatFile,
	sm_path: &Path,
	cache: &SeenCache,
) -> Result<(), SmPackageError> {
	let chart_dir = sm_path.parent().unwrap_or(Path::new("."));
	let bytes = bb.chart.decompress();
	let msd = sm_msd::from_bytes(&bytes);
	let header_elements: Vec<MsdElement> = msd
		.elements
		.into_iter()
		.filter(|el| &*el.tag != b"NOTES")
		.collect();

	resolve_header_assets(bb, chart_dir, &header_elements, cache)?;
	Ok(())
}

fn resolve_header_assets(
	bb: &mut BackbeatFile,
	chart_dir: &Path,
	header: &[MsdElement],
	cache: &SeenCache,
) -> Result<(), SmPackageError> {
	for (real_tag, fallback_tag) in [
		("REALMUSIC", "MUSIC"),
		("REALBANNER", "BANNER"),
		("REALBACKGROUND", "BACKGROUND"),
		("REALJACKET", "JACKET"),
		("REALCDTITLE", "CDTITLE"),
	] {
		let selected = get_tag_value(header, real_tag.as_bytes())
			.map(|path| (real_tag, path))
			.or_else(|| {
				get_tag_value(header, fallback_tag.as_bytes()).map(|path| (fallback_tag, path))
			});
		let Some((tag, rel_path)) = selected else {
			continue;
		};
		let rel_path = std::str::from_utf8(rel_path)
			.map_err(|source| SmPackageError::InvalidAssetPathUtf8 { tag, source })?;
		ensure_asset(bb, chart_dir, cache, rel_path, &[])?;
	}

	if let Some(rel_path) = get_tag_value(header, b"LYRICSPATH") {
		let rel_path = std::str::from_utf8(rel_path).map_err(|source| {
			SmPackageError::InvalidAssetPathUtf8 {
				tag: "LYRICSPATH",
				source,
			}
		})?;
		ensure_asset(bb, chart_dir, cache, rel_path, &[])?;
	}

	Ok(())
}

fn get_tag_value<'a>(header: &'a [MsdElement], tag: &[u8]) -> Option<&'a [u8]> {
	header
		.iter()
		.find(|el| &*el.tag == tag)
		.and_then(|el| el.values.first())
		.map(Box::as_ref)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn make_sm(notes_count: usize) -> Vec<u8> {
		let mut s = String::from(
			"#TITLE:Test Song;\n\
			 #ARTIST:Test Artist;\n\
			 #MUSIC:song.ogg;\n",
		);
		for i in 0..notes_count {
			s.push_str(&format!(
				"#NOTES:\n\
				     dance-single:\n\
				     Author:\n\
				     Hard:\n\
				     {}:\n\
				     0,0,0,0,0:\n\
				0000\n\
				;\n",
				10 + i
			));
		}
		s.into_bytes()
	}

	fn no_cache() -> SeenCache {
		SeenCache::new()
	}

	fn package_bytes(bytes: &[u8], path: &str, cache: &SeenCache) -> BackbeatFile {
		let chart_dir = Path::new(".");
		let chart_path = chart_dir.join(path);
		let mut bb = crate::base::from_bytes(bytes, &chart_path, cache).unwrap();
		resolve_assets(&mut bb, &chart_path, cache).unwrap();
		bb
	}

	#[test]
	fn convert_whole_file_preserves_bytes() {
		let raw = make_sm(3);
		let bb = package_bytes(&raw, "test.sm", &no_cache());
		assert_eq!(bb.filename.extension(), Some("sm"));
		assert_eq!(bb.chart.decompress(), raw);
	}

	#[test]
	fn accepts_non_utf8_outside_selected_asset_paths() {
		let raw = b"#TITLE:\x80;\n#MUSIC:\x80.ogg;\n#REALMUSIC:song.ogg;\n";
		let chart_path = Path::new("test.sm");
		let cache = no_cache();
		let mut bb = crate::base::from_bytes(raw, chart_path, &cache).unwrap();

		resolve_assets(&mut bb, chart_path, &cache).unwrap();

		assert_eq!(
			cache.missing_asset_paths(),
			vec![backbeat_core::AssetPath::from_path("song.ogg").unwrap()]
		);
	}

	#[test]
	fn rejects_non_utf8_real_asset_paths() {
		let raw = b"#MUSIC:song.ogg;\n#REALMUSIC:\x80.ogg;\n";
		let chart_path = Path::new("test.sm");
		let cache = no_cache();
		let mut bb = crate::base::from_bytes(raw, chart_path, &cache).unwrap();

		let error = resolve_assets(&mut bb, chart_path, &cache).unwrap_err();

		assert!(matches!(
			error,
			SmPackageError::InvalidAssetPathUtf8 {
				tag: "REALMUSIC",
				..
			}
		));
	}
}
