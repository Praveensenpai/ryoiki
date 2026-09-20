use super::super::probe::MediaProbe;
use std::fmt::Write;

#[must_use]
pub fn build_single_prompt(raw_name: &str, probe: Option<&MediaProbe>) -> String {
    let probe_context = format_probe_context(probe);
    format!(
        "You are an expert media organizer for Jellyfin. Given this messy torrent/file name:\n\
        \"{raw_name}\"\n\
        {probe_context}\
        CRITICAL INSTRUCTIONS:\n\
        1. Identify remakes/versions: If multiple movies exist with this title, use audio language and duration to identify the EXACT film version.\n\
        2. Release Year is MANDATORY for movies: Provide 4-digit release year for movies in 'year' and in 'clean_name' (e.g. \"Title (YEAR) [Language] [1080p].ext\").\n\
        3. Anime classification: If Japanese anime, animation, or OVA, classify 'media_type' as \"anime\".\n\
        4. Clean title: Strip release groups, websites, resolution, and year from 'title'.\n\
        5. Specials/Extras: For bonus, special, OVA, NCED, NCOP, menu, or extra content, set 'is_extra': true and season: 0 (if series). ALWAYS keep the descriptive special title in 'clean_name' (e.g. \"Title - S00E01 - Menu 01 [Language] [1080p].ext\" or \"Title - S00E01 - NCOP [Language] [1080p].ext\") so files never get colliding filenames.\n\
        6. Multi-season anime / franchise titles: When subsequent seasons have subtitle additions (e.g. \"Non Non Biyori Repeat\", \"Non Non Biyori Nonstop\", \"Kaguya-sama: Love Is War - Ultra Romantic\"), set 'title' to the canonical base franchise name (e.g. \"Non Non Biyori\", \"Kaguya-sama: Love Is War\") so all seasons group under the same series directory in Jellyfin. In 'clean_name', use the canonical title (e.g. \"Non Non Biyori - S02E01 [Japanese] [1080p].mkv\").\n\
        \n\
        Parse and return JSON with keys:\n\
        - media_type: \"movie\", \"show\", or \"anime\"\n\
        - title: clean title without release group, websites, resolution, or year\n\
        - year: integer release year (e.g. 2013) or null\n\
        - season: integer season number or null (if show or anime; use 0 for specials)\n\
        - episode: integer episode number or null (if show or anime)\n\
        - resolution: string e.g. \"1080p\", \"2160p\", \"720p\" or null\n\
        - language: string primary audio language capitalized (e.g. \"Malayalam\", \"Japanese\", \"English\", \"Tamil\", \"Hindi\", \"Multi\") or null\n\
        - is_extra: boolean (true if bonus, special, OVA, NCED, NCOP, or extra content, otherwise false)\n\
        - clean_name: formatted filename with original file extension (e.g. \"Title (2013) [Malayalam] [1080p].mkv\", \"Title - S01E02 [Japanese] [1080p].mkv\", or \"Title - S00E01 - Extra Name [Japanese] [1080p].mkv\")"
    )
}

#[must_use]
pub fn build_batch_prompt(raw_names: &[&str], probe: Option<&MediaProbe>) -> String {
    let probe_context = format_probe_context(probe);
    let mut files_list = String::new();
    for (i, name) in raw_names.iter().enumerate() {
        let _ = writeln!(files_list, "{}. \"{}\"", i + 1, name);
    }

    format!(
        "You are an expert media organizer for Jellyfin. Classify this BATCH of files from the SAME release/series:\n\
        {files_list}\n\
        {probe_context}\
        CRITICAL INSTRUCTIONS:\n\
        1. Keep the main show/movie 'title', 'media_type', 'year', 'season', and 'language' CONSISTENT across all related files. For multi-season franchises (e.g. Season 1, Repeat, Nonstop), unify 'title' under the canonical base franchise name (e.g. \"Non Non Biyori\") across all files.\n\
        2. Deduce episode numbers sequentially (e.g. 01 -> episode 1, 02 -> episode 2).\n\
        3. For specials, OVAs, menus, NCOPs, NCEDs, spots, interviews, or extra content:\n\
           - Set 'is_extra': true.\n\
           - For series specials, set 'season': 0 and assign distinct 'episode' numbers (e.g. S00E01, S00E02).\n\
           - In 'clean_name', ALWAYS include the descriptive extra title so filenames are completely unique (e.g. \"Title - S00E01 - Menu 01 [Language] [Resolution].ext\", \"Title - S00E02 - NCOP 01 [Language] [Resolution].ext\"). NEVER give different special files identical clean_name!\n\
        4. For regular episodes use clean_name: \"Title - S01E01 [Language] [Resolution].ext\".\n\
        5. Return a JSON ARRAY of objects, one for each input file in order:\n\
        [\n\
          {{\n\
            \"raw_name\": \"exact input filename\",\n\
            \"media_type\": \"movie\" | \"show\" | \"anime\",\n\
            \"title\": \"Clean Title\",\n\
            \"year\": integer or null,\n\
            \"season\": integer or null,\n\
            \"episode\": integer or null,\n\
            \"resolution\": \"1080p\" or null,\n\
            \"language\": \"Japanese\" or null,\n\
            \"is_extra\": boolean,\n\
            \"clean_name\": \"Title - S01E01 [Japanese] [1080p].mkv\"\n\
          }}\n\
        ]"
    )
}

#[must_use]
pub fn format_probe_context(probe: Option<&MediaProbe>) -> String {
    let Some(p) = probe else {
        return String::new();
    };

    let mut details = Vec::new();
    if let Some(mins) = p.duration_mins {
        details.push(format!("Exact media duration: ~{mins} minutes"));
    }
    if !p.audio_languages.is_empty() {
        details.push(format!(
            "Audio language tracks detected: {}",
            p.audio_languages.join(", ")
        ));
    }
    if let Some(res) = &p.resolution {
        details.push(format!("Detected stream resolution: {res}"));
    }

    if details.is_empty() {
        String::new()
    } else {
        format!(
            "\nPhysical media file stream probe:\n{}\n\n",
            details
                .iter()
                .map(|d| format!("- {d}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
