use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

use encoding_rs::EUC_KR;

use crate::sm::{Bpm, ChartData, Difficulty, Event, Measure, NoteVariant, StepsType, Stop};
use crate::sm_msd::{self, MsdFile};
use crate::utils::ByteString;

const ROWS_PER_BEAT: usize = 48;
const ROWS_PER_MEASURE: usize = ROWS_PER_BEAT * 4;
const DEFAULT_BPM: f64 = 60.0;
const DEFAULT_TICK_COUNT: usize = 4;

#[derive(Debug, Clone)]
pub struct Chart {
	pub tags: MsdFile,
	pub title: String,
	pub artist: String,
	pub bpms: Vec<Bpm>,
	pub stops: Vec<Stop>,
	pub offset_secs: Option<f64>,
	pub preview_start_ms: Option<u64>,
	pub music: Option<String>,
	pub banner: Option<String>,
	pub background: Option<String>,
	pub preview_music: Option<String>,
	pub description: String,
	pub chart_data: ChartData,
	pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
	#[error("KSF file has no #STEP data")]
	MissingSteps,
	#[error("KSF TICKCOUNT must be greater than zero")]
	InvalidTickCount,
}

pub fn from_path(path: impl AsRef<Path>) -> io::Result<Result<Chart, LoadError>> {
	let path = path.as_ref();
	let bytes = fs::read(path)?;
	Ok(from_bytes(&bytes, path))
}

pub fn from_bytes(bytes: &[u8], path: impl AsRef<Path>) -> Result<Chart, LoadError> {
	let path = path.as_ref();
	let tags = sm_msd::from_bytes_preserving_escapes(bytes);
	let filename = path
		.file_stem()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| "chart".to_owned());
	let filename_lower = filename.to_ascii_lowercase();

	let (mut title, mut artist) = first_value(&tags, b"TITLE")
		.map(decode_text)
		.map(|value| split_title(&value))
		.unwrap_or_default();
	if title.is_empty() || artist.is_empty() {
		let fallback = path
			.parent()
			.and_then(Path::file_name)
			.map(|name| decode_text(name.to_string_lossy().as_bytes()))
			.unwrap_or_default();
		let (fallback_title, fallback_artist) = split_title(&fallback);
		if title.is_empty() {
			title = fallback_title;
		}
		if artist.is_empty() {
			artist = fallback_artist;
		}
	}
	if title.is_empty() {
		title = "Untitled Song".to_owned();
	}
	if artist.is_empty() {
		artist = "Unknown Artist".to_owned();
	}

	let start_time = tag_number(&tags, b"STARTTIME").unwrap_or(0.0);
	let start_time2 = tag_number(&tags, b"STARTTIME2").unwrap_or(0.0);
	let offset_secs = (start_time != 0.0).then_some(-start_time / 100.0);
	let initial_bpm = tag_number(&tags, b"BPM")
		.filter(|bpm| *bpm > 0.0)
		.unwrap_or(DEFAULT_BPM);
	let mut bpms = vec![Bpm {
		bpm: initial_bpm,
		offset_beats: 0.0,
	}];

	let bpm2 = tag_number(&tags, b"BPM2").filter(|bpm| *bpm > 0.0);
	let bpm3 = tag_number(&tags, b"BPM3").filter(|bpm| *bpm > 0.0);
	let bunki = tag_number(&tags, b"BUNKI").filter(|position| *position > 0.0);
	let bunki2 = tag_number(&tags, b"BUNKI2").filter(|position| *position > 0.0);
	if let (Some(bpm), Some(position)) = (bpm2, bunki) {
		bpms.push(Bpm {
			bpm,
			offset_beats: (position / 100.0 - start_time / 100.0) * initial_bpm / 60.0,
		});
	}
	if let (Some(bpm), Some(position), Some(previous_bpm)) = (bpm3, bunki2, bpm2) {
		bpms.push(Bpm {
			bpm,
			offset_beats: (position / 100.0 - start_time2 / 100.0) * previous_bpm / 60.0,
		});
	}

	let tick_count = parse_tick_count(&tags)?;
	let step_data = first_value(&tags, b"STEP").ok_or(LoadError::MissingSteps)?;
	let player_is_double = tags.elements.iter().any(|element| {
		element.tag.as_ref() == b"PLAYER"
			&& scalar_value(element)
				.is_some_and(|value| decode_text(value).to_ascii_lowercase().contains("double"))
	});
	let steps_type = infer_steps_type(&filename_lower, player_is_double);
	let difficulty = infer_difficulty(&filename_lower);
	let level = last_scalar_value(&tags, b"DIFFICULTY")
		.map(|value| parse_stepmania_integer(value).max(1) as usize)
		.unwrap_or_else(|| default_level(&filename_lower));
	let (notedata, direct_bpms, stops) =
		parse_notes(step_data, &steps_type, tick_count, initial_bpm);
	bpms.extend(direct_bpms);
	bpms.sort_by(|left, right| left.offset_beats.total_cmp(&right.offset_beats));

	let chart_data = ChartData {
		steps_type,
		author: ByteString::default(),
		difficulty,
		level,
		notedata,
	};
	let preview_start_ms = last_scalar_value(&tags, b"MUSICINTRO")
		.or_else(|| last_scalar_value(&tags, b"INTRO"))
		.and_then(parse_timestamp_ms);
	let music = asset_value(&tags, b"SONGFILE").or_else(|| default_music(path));
	let banner = asset_value(&tags, b"DISCFILE");
	let background = asset_value(&tags, b"TITLEFILE");
	let preview_music = asset_value(&tags, b"INTROFILE");

	Ok(Chart {
		tags,
		title,
		artist,
		bpms,
		stops,
		offset_secs,
		preview_start_ms,
		music,
		banner,
		background,
		preview_music,
		description: filename,
		chart_data,
		path: path.to_path_buf(),
	})
}

fn first_value<'a>(tags: &'a MsdFile, tag: &[u8]) -> Option<&'a [u8]> {
	tags.elements
		.iter()
		.find(|element| element.tag.as_ref() == tag)
		.and_then(|element| element.values.first())
		.map(Box::as_ref)
		.map(<[u8]>::trim_ascii)
		.filter(|value| !value.is_empty())
}

fn decode_text(bytes: &[u8]) -> String {
	let (value, _, had_errors) = EUC_KR.decode(bytes);
	if had_errors {
		String::new()
	} else {
		value.trim().to_owned()
	}
}

fn split_title(value: &str) -> (String, String) {
	let mut parts = value.split(" - ").map(str::trim).collect::<Vec<_>>();
	if parts.len() == 3 && is_difficulty_name(parts[2]) {
		parts.pop();
	}
	match parts.as_slice() {
		[artist, title] => ((*title).to_owned(), (*artist).to_owned()),
		[title] => ((*title).to_owned(), String::new()),
		_ => (String::new(), String::new()),
	}
}

fn is_difficulty_name(value: &str) -> bool {
	["double", "easy", "normal", "hard", "crazy", "nightmare"]
		.iter()
		.any(|difficulty| value.eq_ignore_ascii_case(difficulty))
}

fn scalar_value(element: &crate::sm_msd::MsdElement) -> Option<&[u8]> {
	element
		.values
		.first()
		.map(Box::as_ref)
		.map(<[u8]>::trim_ascii)
}

fn last_scalar_value<'a>(tags: &'a MsdFile, tag: &[u8]) -> Option<&'a [u8]> {
	tags.elements
		.iter()
		.rev()
		.find(|element| element.tag.as_ref() == tag)
		.and_then(scalar_value)
}

fn parse_stepmania_integer(value: &[u8]) -> i32 {
	let Ok(value) = std::str::from_utf8(value) else {
		return 0;
	};
	lexical::parse_partial::<i32, _>(value)
		.ok()
		.map_or(0, |(number, _)| number)
}

fn parse_tick_count(tags: &MsdFile) -> Result<usize, LoadError> {
	let mut tick_count = None;
	for element in &tags.elements {
		if element.tag.as_ref() != b"TICKCOUNT" {
			continue;
		}
		let value = scalar_value(element).map_or(0, parse_stepmania_integer);
		if value <= 0 {
			return Err(LoadError::InvalidTickCount);
		}
		tick_count = Some(value as usize);
	}
	Ok(tick_count.unwrap_or(DEFAULT_TICK_COUNT))
}

fn tag_number(tags: &MsdFile, tag: &[u8]) -> Option<f64> {
	let value = std::str::from_utf8(last_scalar_value(tags, tag)?).ok()?;
	let (number, _) = lexical::parse_partial::<f64, _>(value).ok()?;
	number.is_finite().then_some(number)
}

fn asset_value(tags: &MsdFile, tag: &[u8]) -> Option<String> {
	last_scalar_value(tags, tag)
		.map(decode_text)
		.filter(|value| !value.is_empty())
}

fn default_music(path: &Path) -> Option<String> {
	let directory = path.parent().unwrap_or(Path::new("."));
	["song.mp3", "song.oga", "song.ogg", "song.wav"]
		.into_iter()
		.find(|name| directory.join(name).is_file())
		.map(str::to_owned)
}

fn parse_timestamp_ms(value: &[u8]) -> Option<u64> {
	let value = std::str::from_utf8(value).ok()?.trim();
	let parts = value
		.split(':')
		.map(|part| part.trim().parse::<f64>().ok())
		.collect::<Option<Vec<_>>>()?;
	let seconds = match parts.as_slice() {
		[seconds] => *seconds,
		[minutes, seconds] => minutes.mul_add(60.0, *seconds),
		[hours, minutes, seconds] => hours.mul_add(3600.0, minutes.mul_add(60.0, *seconds)),
		_ => return None,
	};
	(seconds.is_finite() && seconds >= 0.0).then_some((seconds * 1000.0).round() as u64)
}

fn infer_steps_type(filename: &str, player_is_double: bool) -> StepsType {
	if contains_any(filename, &["halfdouble", "half-double", "h_double", "hdb"]) {
		StepsType::PumpHalfDouble
	} else if contains_any(
		filename,
		&["double", "nightmare", "freestyle", "db", "nm", "fs"],
	) || player_is_double
	{
		StepsType::PumpDouble
	} else if filename.contains("_2") {
		StepsType::PumpCouple
	} else {
		StepsType::PumpSingle
	}
}

fn infer_difficulty(filename: &str) -> Difficulty {
	if filename.contains("another") {
		Difficulty::Edit(ByteString::default())
	} else if contains_any(filename, &["wild", "wd", "crazy+", "cz+", "hardcore"]) {
		Difficulty::Challenge
	} else if contains_any(filename, &["crazy", "cz", "nightmare", "nm", "crazydouble"]) {
		Difficulty::Hard
	} else if contains_any(filename, &["hard", "hd", "freestyle", "fs", "double"]) {
		Difficulty::Medium
	} else if contains_any(filename, &["easy", "ez", "normal"]) {
		Difficulty::Easy
	} else if contains_any(filename, &["beginner", "practice", "pr"]) {
		Difficulty::Beginner
	} else {
		Difficulty::Hard
	}
}

fn default_level(filename: &str) -> usize {
	if filename.contains("another") {
		25
	} else if contains_any(filename, &["wild", "wd", "crazy+", "cz+", "hardcore"]) {
		20
	} else if contains_any(filename, &["crazy", "cz", "nightmare", "nm", "crazydouble"]) {
		14
	} else if contains_any(filename, &["hard", "hd", "freestyle", "fs", "double"]) {
		8
	} else if contains_any(
		filename,
		&["easy", "ez", "normal", "beginner", "practice", "pr"],
	) {
		4
	} else {
		10
	}
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
	needles.iter().any(|needle| value.contains(needle))
}

struct NoteDataBuilder {
	columns: Vec<BTreeMap<usize, NoteVariant>>,
	hold_starts: Vec<Option<usize>>,
	last_row: usize,
	end_row: usize,
}

impl NoteDataBuilder {
	fn new(columns: usize) -> Self {
		Self {
			columns: (0..columns).map(|_| BTreeMap::new()).collect(),
			hold_starts: vec![None; columns],
			last_row: 0,
			end_row: 0,
		}
	}

	fn write(&mut self, column: usize, row: usize, variant: NoteVariant) {
		if let Some(events) = self.columns.get_mut(column) {
			events.insert(row, variant);
		}
	}

	fn close_hold(&mut self, column: usize) {
		let Some(start) = self.hold_starts.get_mut(column).and_then(Option::take) else {
			return;
		};
		if start == self.last_row {
			self.write(column, start, NoteVariant::Note);
		} else {
			self.write(column, start, NoteVariant::HoldStart);
			self.write(column, self.last_row, NoteVariant::HoldOrRollEnd);
		}
	}

	fn finish(mut self) -> Vec<Measure> {
		for column in 0..self.columns.len() {
			self.close_hold(column);
		}
		let event_end = self
			.columns
			.iter()
			.filter_map(|events| events.last_key_value().map(|(row, _)| row + 1))
			.max()
			.unwrap_or(0);
		let measure_count = self.end_row.max(event_end).div_ceil(ROWS_PER_MEASURE);
		let mut measures = (0..measure_count)
			.map(|_| Measure {
				size: ROWS_PER_MEASURE,
				events: Vec::new(),
			})
			.collect::<Vec<_>>();
		for (column, events) in self.columns.into_iter().enumerate() {
			for (row, variant) in events {
				if let Some(measure) = measures.get_mut(row / ROWS_PER_MEASURE) {
					measure.events.push(Event {
						row: row % ROWS_PER_MEASURE,
						column,
						variant,
					});
				}
			}
		}
		for measure in &mut measures {
			measure
				.events
				.sort_by_key(|event| (event.row, event.column));
		}
		measures
	}
}

fn parse_notes(
	step_data: &[u8],
	steps_type: &StepsType,
	initial_tick_count: usize,
	initial_bpm: f64,
) -> (Vec<Measure>, Vec<Bpm>, Vec<Stop>) {
	let column_count = match steps_type {
		StepsType::PumpSingle => 5,
		StepsType::PumpHalfDouble => 6,
		StepsType::PumpDouble | StepsType::PumpCouple | StepsType::PumpRoutine => 10,
		_ => 5,
	};
	let mut builder = NoteDataBuilder::new(column_count);
	let mut tick_count = initial_tick_count;
	let mut beat = 0.0_f64;
	let mut current_bpm = initial_bpm;
	let mut bpms = Vec::new();
	let mut stops = Vec::new();

	for raw_line in step_data.split(|byte| *byte == b'\n') {
		let mut line = raw_line.trim_ascii();
		if line.is_empty() {
			continue;
		}
		if line == b"2222222222222" {
			break;
		}
		if line.starts_with(b"|") {
			if let Some((command, value)) = direct_move_command(line) {
				match command {
					b'T' => {
						if let Some(value) = parse_ascii_number(value)
							.map(|value| value as usize)
							.filter(|value| *value > 0)
						{
							tick_count = value;
						}
					}
					b'B' => {
						if let Some(value) = parse_ascii_number(value).filter(|value| *value > 0.0)
						{
							current_bpm = value;
							bpms.push(Bpm {
								bpm: value,
								offset_beats: beat,
							});
						}
					}
					b'D' => {
						if let Some(value) = parse_ascii_number(value).filter(|value| *value > 0.0)
						{
							stops.push(Stop {
								duration: value / 1000.0,
								offset_beats: beat,
							});
						}
					}
					b'E' => {
						if let Some(value) = parse_ascii_number(value).filter(|value| *value > 0.0)
						{
							stops.push(Stop {
								duration: 60.0 / current_bpm * value / tick_count as f64,
								offset_beats: beat,
							});
						}
					}
					_ => {}
				}
			}
			continue;
		}

		if matches!(steps_type, StepsType::PumpHalfDouble) {
			line = line.get(2..).unwrap_or_default();
		}
		let row = (beat * ROWS_PER_BEAT as f64).round() as usize;
		for column in 0..column_count {
			let note = line.get(column).copied().unwrap_or(b'0');
			if note == b'4' {
				if builder.hold_starts[column].is_none() {
					builder.hold_starts[column] = Some(row);
				}
				continue;
			}
			builder.close_hold(column);
			let variant = match note {
				b'1' => Some(NoteVariant::Note),
				b'M' | b'm' => Some(NoteVariant::Mine),
				b'F' | b'f' => Some(NoteVariant::Fake),
				b'L' | b'l' => Some(NoteVariant::Lift),
				_ => None,
			};
			if let Some(variant) = variant {
				builder.write(column, row, variant);
			}
		}
		builder.last_row = row;
		beat += 1.0 / tick_count as f64;
		builder.end_row = (beat * ROWS_PER_BEAT as f64).round() as usize;
	}

	(builder.finish(), bpms, stops)
}

fn direct_move_command(line: &[u8]) -> Option<(u8, &[u8])> {
	let line = line.strip_prefix(b"|")?.strip_suffix(b"|")?;
	Some((*line.first()?, line.get(1..)?.trim_ascii()))
}

fn parse_ascii_number(value: &[u8]) -> Option<f64> {
	let value = std::str::from_utf8(value).ok()?;
	let (number, _) = lexical::parse_partial::<f64, _>(value).ok()?;
	number.is_finite().then_some(number)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_stepmania_sample() {
		let bytes = b"#TITLE:Example Artist - Example Song - Crazy;\r\n#BPM:150;\r\n#STARTTIME:25;\r\n#PLAYER:SINGLE;\r\n#TICKCOUNT:4;\r\n#SONGFILE:song.ogg;\r\n#DISCFILE:disc.bmp;\r\n#TITLEFILE:back.bmp;\r\n#DIFFICULTY:12;\r\n#STEP:\r\n1000000000000\r\n0040000000000\r\n0040000000000\r\n0000000000000\r\n2222222222222";
		let chart = from_bytes(bytes, "crazy_1.ksf").unwrap();
		assert_eq!(chart.title, "Example Song");
		assert_eq!(chart.artist, "Example Artist");
		assert_eq!(chart.offset_secs, Some(-0.25));
		assert_eq!(chart.music.as_deref(), Some("song.ogg"));
		assert_eq!(chart.chart_data.steps_type, StepsType::PumpSingle);
		assert_eq!(chart.chart_data.difficulty, Difficulty::Hard);
		assert_eq!(chart.chart_data.level, 12);
		assert_eq!(chart.chart_data.notedata.len(), 1);
		assert!(
			chart.chart_data.notedata[0]
				.events
				.iter()
				.any(|event| event.variant == NoteVariant::HoldStart)
		);
		assert!(
			chart.chart_data.notedata[0]
				.events
				.iter()
				.any(|event| event.variant == NoteVariant::HoldOrRollEnd)
		);
	}

	#[test]
	fn infers_pump_modes_and_difficulties_from_filename() {
		let chart = from_bytes(b"#STEP:\n0000000000000\n2222222222222", "halfdouble.ksf").unwrap();
		assert_eq!(chart.chart_data.steps_type, StepsType::PumpHalfDouble);
		assert_eq!(chart.chart_data.difficulty, Difficulty::Medium);
		assert_eq!(chart.chart_data.level, 8);

		let chart = from_bytes(
			b"#PLAYER:DOUBLE;#STEP:\n0000000000000\n2222222222222",
			"custom.ksf",
		)
		.unwrap();
		assert_eq!(chart.chart_data.steps_type, StepsType::PumpDouble);
	}

	#[test]
	fn parses_bunki_and_direct_move_timing() {
		let chart = from_bytes(
			b"#BPM:120;#BPM2:180;#BUNKI:1000;#TICKCOUNT:4;#STEP:\n|B240|\n|D500|\n1000000000000\n2222222222222",
			"normal_1.ksf",
		)
		.unwrap();
		assert_eq!(chart.bpms.len(), 3);
		assert_eq!(chart.bpms[1].offset_beats, 0.0);
		assert_eq!(chart.bpms[1].bpm, 240.0);
		assert_eq!(chart.bpms[2].offset_beats, 20.0);
		assert_eq!(chart.stops[0].duration, 0.5);
	}

	#[test]
	fn decodes_legacy_korean_metadata() {
		let (encoded, _, had_errors) = EUC_KR.encode("아티스트 - 노래");
		assert!(!had_errors);
		let mut bytes = b"#TITLE:".to_vec();
		bytes.extend_from_slice(&encoded);
		bytes.extend_from_slice(b";#STEP:\n0000000000000\n2222222222222");

		let chart = from_bytes(&bytes, "normal_1.ksf").unwrap();
		assert_eq!(chart.artist, "아티스트");
		assert_eq!(chart.title, "노래");
	}

	#[test]
	fn valid_utf8_korean_is_not_accepted_as_cp949() {
		assert!(decode_text("노래".as_bytes()).is_empty());
	}

	#[test]
	fn explicit_empty_or_invalid_difficulty_becomes_one() {
		for difficulty in [b"".as_ref(), b"not-a-number"] {
			let mut bytes = b"#DIFFICULTY:".to_vec();
			bytes.extend_from_slice(difficulty);
			bytes.extend_from_slice(b";#STEP:\n0000000000000\n2222222222222");
			let chart = from_bytes(&bytes, "crazy_1.ksf").unwrap();
			assert_eq!(chart.chart_data.level, 1);
		}
	}

	#[test]
	fn later_difficulty_replaces_earlier_values() {
		let chart = from_bytes(
			b"#DIFFICULTY:3;#DIFFICULTY:12;#STEP:\n0000000000000\n2222222222222",
			"normal_1.ksf",
		)
		.unwrap();
		assert_eq!(chart.chart_data.level, 12);
	}

	#[test]
	fn rejects_non_positive_empty_or_invalid_tickcount() {
		for tick_count in [b"0".as_ref(), b"-1", b"", b"not-a-number"] {
			let mut bytes = b"#TICKCOUNT:".to_vec();
			bytes.extend_from_slice(tick_count);
			bytes.extend_from_slice(b";#STEP:\n2222222222222");
			assert!(matches!(
				from_bytes(&bytes, "normal_1.ksf"),
				Err(LoadError::InvalidTickCount)
			));
		}
	}
}
