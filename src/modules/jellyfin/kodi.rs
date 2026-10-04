use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

const KODI_ADVANCED_SETTINGS: &str = r#"<advancedsettings version="1.0">
  <network>
    <buffermode>1</buffermode>
    <memorysize>134217728</memorysize>
    <readfactor>20</readfactor>
    <curlclienttimeout>60</curlclienttimeout>
  </network>
</advancedsettings>
"#;

#[derive(Subcommand, Debug, Clone)]
pub enum KodiSubcommand {
    /// Show recommended Kodi advancedsettings.xml and setup guide
    Config {
        /// Print only raw XML (suitable for piping to a file)
        #[arg(long)]
        raw: bool,

        /// Save advancedsettings.xml directly to the specified file path
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Print seeking and network troubleshooting tips for jellyfin-kodi
    Guide,
}

pub fn handle_cli(sub: &KodiSubcommand) -> Result<()> {
    match sub {
        KodiSubcommand::Config { raw, out } => {
            if let Some(dest) = out {
                fs::write(dest, KODI_ADVANCED_SETTINGS)
                    .with_context(|| format!("Failed to write to {}", dest.display()))?;
                println!(
                    "  {} Generated {}",
                    "✔".green().bold(),
                    dest.display().to_string().cyan()
                );
            } else if *raw {
                print!("{KODI_ADVANCED_SETTINGS}");
            } else {
                print_config_guide();
            }
        }
        KodiSubcommand::Guide => {
            print_troubleshooting_guide();
        }
    }
    Ok(())
}

fn print_config_guide() {
    println!("\n  {} {}", "🎬".bold(), "Kodi Streaming Configuration".bold());
    println!("  {}\n", "─".repeat(45).dimmed());

    println!("  Place this content in your Kodi {} file:\n", "userdata/advancedsettings.xml".cyan());
    for line in KODI_ADVANCED_SETTINGS.lines() {
        println!("    {line}");
    }

    println!("\n  {} Standard userdata locations:", "📁".bold());
    println!("    • Android:     /sdcard/Android/data/org.xbmc.kodi/files/.kodi/userdata/");
    println!("    • Linux:       ~/.kodi/userdata/");
    println!("    • LibreELEC:   /storage/.kodi/userdata/");
    println!("    • Windows:     %APPDATA%\\Kodi\\userdata\\");

    println!("\n  {} Next Step:", "💡".bold());
    println!("    Pipe directly to file: {}", "ryoiki kodi config --raw > advancedsettings.xml".yellow());
    println!();
}

fn print_troubleshooting_guide() {
    println!("\n  {} {}", "🛠️".bold(), "Jellyfin + Kodi Playback Optimization".bold());
    println!("  {}\n", "─".repeat(45).dimmed());

    println!("  {} Long Seek Abort Fix (jellyfin-kodi):", "1.".bold());
    println!("    • In Kodi: Settings -> Add-ons -> My add-ons -> Video add-ons -> Jellyfin");
    println!("    • Select 'Settings' -> 'Playback' or 'Advanced'");
    println!("    • Toggle {} to {} (fixes HTTP/2 stream cancellation on deep seeks)", "Enable HTTP/2".cyan(), "OFF".bold().red());

    println!("\n  {} RAM Buffer Underrun Fix:", "2.".bold());
    println!("    • Run {} and deploy the XML.", "ryoiki kodi config".yellow());
    println!("    • Expands Kodi RAM buffer to 128 MB and sets curl timeout to 60s.");

    println!("\n  {} Playback Engine Modes:", "3.".bold());
    println!("    • Add-on Mode:  Proxies through Python HTTP addon.");
    println!("    • Native Mode:  Direct paths (SMB/NFS) played by Kodi native C++ engine.");
    println!("    • JellyCon:     Lightweight alternative addon with zero SQLite overhead.");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kodi_advanced_settings_contains_network_tags() {
        assert!(KODI_ADVANCED_SETTINGS.contains("<buffermode>1</buffermode>"));
        assert!(KODI_ADVANCED_SETTINGS.contains("<memorysize>134217728</memorysize>"));
        assert!(KODI_ADVANCED_SETTINGS.contains("<readfactor>20</readfactor>"));
        assert!(KODI_ADVANCED_SETTINGS.contains("<curlclienttimeout>60</curlclienttimeout>"));
    }
}
