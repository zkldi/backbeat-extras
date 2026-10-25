use rg_formats::ksf::Chart;

use crate::types::{AssetPathExtract, ChartExtract, ChartMetaExtract, SongExtract};
use crate::util::{path_string, text};

pub(crate) fn extract(chart: &Chart) -> ChartExtract {
	ChartExtract {
		song: SongExtract {
			title: text(&chart.title),
			subtitle: None,
			artist: text(&chart.artist),
			genre: None,
			preview: chart.preview_start_ms,
			title_translit: None,
			artist_translit: None,
		},
		chart: ChartMetaExtract {
			credit: None,
			charter_note: None,
			difficulty_label: Some(chart.chart_data.difficulty.to_string()),
			level: Some(chart.chart_data.level.to_string()),
			chart_name: text(&chart.description),
		},
		assets: AssetPathExtract {
			music: chart.music.as_deref().and_then(path_string),
			banner: chart.banner.as_deref().and_then(path_string),
			background: chart.background.as_deref().and_then(path_string),
			jacket: None,
			signature: None,
		},
	}
}
