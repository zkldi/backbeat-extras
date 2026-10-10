/// Make a portable filename stem, leaving room for an extension and collision suffix.
pub(crate) fn sanitise_filename(name: &str) -> String {
	let mut sanitised: String = name
		.chars()
		.map(|character| {
			if character.is_control()
				|| matches!(
					character,
					'/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
				) {
				'-'
			} else {
				character
			}
		})
		.collect();

	while sanitised.len() > 200 {
		sanitised.pop();
	}
	sanitised = sanitised.trim_end_matches([' ', '.']).to_owned();
	if sanitised.is_empty() || matches!(sanitised.as_str(), "." | "..") {
		return "something".to_owned();
	}

	let stem = sanitised
		.split('.')
		.next()
		.unwrap_or_default()
		.trim_end_matches([' ', '.']);
	if is_windows_device_name(stem) {
		let index = sanitised.find('.').unwrap_or(sanitised.len());
		sanitised.insert(index, '-');
	}

	sanitised
}

fn is_windows_device_name(stem: &str) -> bool {
	let uppercase = stem.to_ascii_uppercase();
	matches!(
		uppercase.as_str(),
		"CON"
			| "PRN" | "AUX"
			| "NUL" | "COM1"
			| "COM2" | "COM3"
			| "COM4" | "COM5"
			| "COM6" | "COM7"
			| "COM8" | "COM9"
			| "LPT1" | "LPT2"
			| "LPT3" | "LPT4"
			| "LPT5" | "LPT6"
			| "LPT7" | "LPT8"
			| "LPT9" | "CLOCK$"
			| "COM¹" | "COM²"
			| "COM³" | "LPT¹"
			| "LPT²" | "LPT³"
	)
}
