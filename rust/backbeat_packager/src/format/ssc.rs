//! Package `.ssc` files into a [`BackbeatFile`].

use std::io;
use std::path::Path;

use rg_formats::sm_msd::{self, MsdElement};

use crate::seen_cache::SeenCache;
use backbeat_core::{BackbeatFile, BackbeatFileError};

use crate::deps::ensure_asset;

/// Errors that can occur when packaging an SSC file into a [`BackbeatFile`].
#[derive(Debug, thiserror::Error)]
pub enum SscPackageError {
	#[error(transparent)]
	FromFile(#[from] crate::error::FromFileError),

	#[error("SSC asset path in #{tag} is not valid UTF-8: {source}")]
	InvalidAssetPathUtf8 {
		tag: &'static str,
		#[source]
		source: std::str::Utf8Error,
	},

	#[error("could not build BackbeatFile: {0}")]
	Build(#[from] BackbeatFileError),

	#[error("could not resolve SSC assets: {0}")]
	Io(#[from] io::Error),
}

pub(crate) fn resolve_assets(
	bb: &mut BackbeatFile,
	ssc_path: &Path,
	cache: &SeenCache,
) -> Result<(), SscPackageError> {
	let chart_dir = ssc_path.parent().unwrap_or(Path::new("."));
	let bytes = bb.chart.decompress();
	let msd = sm_msd::from_bytes(&bytes);
	let elements = msd.elements;

	let split_pos = elements
		.iter()
		.position(|el| &*el.tag == b"NOTEDATA")
		.unwrap_or(elements.len());

	let (song_elements, chart_stream) = elements.split_at(split_pos);
	resolve_msd_assets(bb, chart_dir, song_elements, chart_stream, cache)?;
	Ok(())
}

fn resolve_msd_assets(
	bb: &mut BackbeatFile,
	chart_dir: &Path,
	song_elements: &[MsdElement],
	chart_stream: &[MsdElement],
	cache: &SeenCache,
) -> Result<(), SscPackageError> {
	for (real_tag, fallback_tag) in [
		("REALMUSIC", "MUSIC"),
		("REALBANNER", "BANNER"),
		("REALBACKGROUND", "BACKGROUND"),
		("REALJACKET", "JACKET"),
		("REALCDTITLE", "CDTITLE"),
	] {
		let selected = get_tag_value(song_elements, real_tag.as_bytes())
			.map(|path| (real_tag, path))
			.or_else(|| {
				get_tag_value(song_elements, fallback_tag.as_bytes())
					.map(|path| (fallback_tag, path))
			});
		let Some((tag, path)) = selected else {
			continue;
		};
		let path = utf8_asset_path(tag, path)?;
		ensure_asset(bb, chart_dir, cache, path, &[])?;
	}

	if let Some(path) = get_tag_value(song_elements, b"LYRICSPATH") {
		let path = utf8_asset_path("LYRICSPATH", path)?;
		ensure_asset(bb, chart_dir, cache, path, &[])?;
	}

	// A song-level REALMUSIC is the enriched, definitive resolution. Only inspect
	// per-chart legacy MUSIC tags when enrichment did not provide one.
	if get_tag_value(song_elements, b"REALMUSIC").is_none() {
		for el in chart_stream {
			if &*el.tag == b"MUSIC"
				&& let Some(val) = el.values.first()
			{
				let path = utf8_asset_path("MUSIC", val)?;
				ensure_asset(bb, chart_dir, cache, path, &[])?;
			}
		}
	}

	Ok(())
}

fn utf8_asset_path<'a>(tag: &'static str, path: &'a [u8]) -> Result<&'a str, SscPackageError> {
	std::str::from_utf8(path)
		.map_err(|source| SscPackageError::InvalidAssetPathUtf8 { tag, source })
}

fn get_tag_value<'a>(elements: &'a [MsdElement], tag: &[u8]) -> Option<&'a [u8]> {
	elements
		.iter()
		.find(|el| &*el.tag == tag)
		.and_then(|el| el.values.first())
		.map(Box::as_ref)
		.filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
	use super::*;

	fn make_ssc(charts: &[(&str, &str, usize)]) -> Vec<u8> {
		let mut s = String::from(
			"#TITLE:Test Song;\n\
			 #ARTIST:Test Artist;\n\
			 #MUSIC:song.ogg;\n\
			 #BPMS:0.000=120.000;\n",
		);
		for (steps_type, diff, level) in charts {
			s.push_str(&format!(
				"#NOTEDATA:;\n\
				 #STEPSTYPE:{steps_type};\n\
				 #DESCRIPTION:;\n\
				 #DIFFICULTY:{diff};\n\
				 #METER:{level};\n\
				 #NOTES:\n\
				 0000\n\
				 ;\n"
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
		let raw = make_ssc(&[("dance-single", "easy", 3), ("dance-single", "hard", 8)]);
		let bb = package_bytes(&raw, "test.ssc", &no_cache());
		assert_eq!(bb.filename.extension(), Some("ssc"));
		assert_eq!(bb.chart.decompress(), raw);
	}

	#[test]
	fn accepts_non_utf8_outside_selected_asset_paths() {
		let raw = b"#TITLE:\x80;\n#MUSIC:\x80.ogg;\n#REALMUSIC:song.ogg;\n#NOTEDATA:;\n#MUSIC:\x81.ogg;\n";
		let chart_path = Path::new("test.ssc");
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
		let chart_path = Path::new("test.ssc");
		let cache = no_cache();
		let mut bb = crate::base::from_bytes(raw, chart_path, &cache).unwrap();

		let error = resolve_assets(&mut bb, chart_path, &cache).unwrap_err();

		assert!(matches!(
			error,
			SscPackageError::InvalidAssetPathUtf8 {
				tag: "REALMUSIC",
				..
			}
		));
	}
}
