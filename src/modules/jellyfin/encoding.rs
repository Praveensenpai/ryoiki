use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const DEFAULT_ENCODING_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<EncodingOptions xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <EncodingThreadCount>-1</EncodingThreadCount>
  <EnableFallbackFont>false</EnableFallbackFont>
  <EnableAudioVbr>false</EnableAudioVbr>
  <DownMixAudioBoost>2</DownMixAudioBoost>
  <DownMixStereoAlgorithm>None</DownMixStereoAlgorithm>
  <MaxMuxingQueueSize>2048</MaxMuxingQueueSize>
  <EnableThrottling>false</EnableThrottling>
  <ThrottleDelaySeconds>180</ThrottleDelaySeconds>
  <EnableSegmentDeletion>false</EnableSegmentDeletion>
  <SegmentKeepSeconds>720</SegmentKeepSeconds>
  <HardwareAccelerationType>qsv</HardwareAccelerationType>
  <EncoderAppPathDisplay>/usr/lib/jellyfin-ffmpeg/ffmpeg</EncoderAppPathDisplay>
  <VaapiDevice>/dev/dri/renderD128</VaapiDevice>
  <QsvDevice />
  <EnableTonemapping>true</EnableTonemapping>
  <EnableVppTonemapping>false</EnableVppTonemapping>
  <EnableVideoToolboxTonemapping>false</EnableVideoToolboxTonemapping>
  <TonemappingAlgorithm>bt2390</TonemappingAlgorithm>
  <TonemappingMode>auto</TonemappingMode>
  <TonemappingRange>auto</TonemappingRange>
  <TonemappingDesat>0</TonemappingDesat>
  <TonemappingPeak>100</TonemappingPeak>
  <TonemappingParam>0</TonemappingParam>
  <VppTonemappingBrightness>16</VppTonemappingBrightness>
  <VppTonemappingContrast>1</VppTonemappingContrast>
  <H264Crf>23</H264Crf>
  <H265Crf>28</H265Crf>
  <EncoderPreset>auto</EncoderPreset>
  <DeinterlaceDoubleRate>false</DeinterlaceDoubleRate>
  <DeinterlaceMethod>yadif</DeinterlaceMethod>
  <EnableDecodingColorDepth10Hevc>true</EnableDecodingColorDepth10Hevc>
  <EnableDecodingColorDepth10Vp9>true</EnableDecodingColorDepth10Vp9>
  <EnableDecodingColorDepth10HevcRext>false</EnableDecodingColorDepth10HevcRext>
  <EnableDecodingColorDepth12HevcRext>false</EnableDecodingColorDepth12HevcRext>
  <EnableEnhancedNvdecDecoder>true</EnableEnhancedNvdecDecoder>
  <PreferSystemNativeHwDecoder>true</PreferSystemNativeHwDecoder>
  <EnableIntelLowPowerH264HwEncoder>false</EnableIntelLowPowerH264HwEncoder>
  <EnableIntelLowPowerHevcHwEncoder>false</EnableIntelLowPowerHevcHwEncoder>
  <EnableHardwareEncoding>true</EnableHardwareEncoding>
  <AllowHevcEncoding>false</AllowHevcEncoding>
  <AllowAv1Encoding>false</AllowAv1Encoding>
  <EnableSubtitleExtraction>true</EnableSubtitleExtraction>
  <SubtitleExtractionTimeoutMinutes>30</SubtitleExtractionTimeoutMinutes>
  <HardwareDecodingCodecs>
    <string>h264</string>
    <string>hevc</string>
    <string>vp9</string>
    <string>vc1</string>
  </HardwareDecodingCodecs>
  <AllowOnDemandMetadataBasedKeyframeExtractionForExtensions>
    <string>mkv</string>
  </AllowOnDemandMetadataBasedKeyframeExtractionForExtensions>
</EncodingOptions>
"#;

/// Provisions or patches Jellyfin's encoding.xml with hardware acceleration options.
pub fn provision_hardware_encoding(base_dir: &Path, dry_run: bool) -> Result<()> {
    if !Path::new("/dev/dri").exists() {
        return Ok(());
    }

    let config_dir = base_dir.join("config/config");
    let target_file = config_dir.join("encoding.xml");

    if dry_run {
        println!("  • [dry-run] Provision {}", target_file.display());
        return Ok(());
    }

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create {}", config_dir.display()))?;
    }

    if !target_file.exists() {
        fs::write(&target_file, DEFAULT_ENCODING_XML)
            .with_context(|| format!("Failed to write {}", target_file.display()))?;
        return Ok(());
    }

    let current = fs::read_to_string(&target_file)
        .with_context(|| format!("Failed to read {}", target_file.display()))?;
    let updated = patch_encoding_xml(&current);

    if updated != current {
        fs::write(&target_file, updated)
            .with_context(|| format!("Failed to update {}", target_file.display()))?;
    }

    Ok(())
}

/// Injects missing HEVC and VP9 codecs into `HardwareDecodingCodecs` if absent.
pub fn patch_encoding_xml(content: &str) -> String {
    let mut result = content.to_string();

    if !result.contains("<HardwareDecodingCodecs>") {
        let replacement = "  <HardwareDecodingCodecs>\n    <string>h264</string>\n    <string>hevc</string>\n    <string>vp9</string>\n    <string>vc1</string>\n  </HardwareDecodingCodecs>\n</EncodingOptions>";
        return result.replace("</EncodingOptions>", replacement);
    }

    if !result.contains("<string>hevc</string>") {
        result = result.replace(
            "<HardwareDecodingCodecs>",
            "<HardwareDecodingCodecs>\n    <string>hevc</string>",
        );
    }

    if !result.contains("<string>vp9</string>") {
        result = result.replace(
            "<HardwareDecodingCodecs>",
            "<HardwareDecodingCodecs>\n    <string>vp9</string>",
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_encoding_xml_inserts_missing_codecs() {
        let xml = r"<EncodingOptions>
  <HardwareDecodingCodecs>
    <string>h264</string>
  </HardwareDecodingCodecs>
</EncodingOptions>";

        let patched = patch_encoding_xml(xml);
        assert!(patched.contains("<string>hevc</string>"));
        assert!(patched.contains("<string>vp9</string>"));
    }

    #[test]
    fn test_patch_encoding_xml_idempotent() {
        let xml = r"<EncodingOptions>
  <HardwareDecodingCodecs>
    <string>h264</string>
    <string>hevc</string>
    <string>vp9</string>
  </HardwareDecodingCodecs>
</EncodingOptions>";

        let patched = patch_encoding_xml(xml);
        assert_eq!(patched, xml);
    }

    #[test]
    fn test_patch_encoding_xml_creates_block_if_absent() {
        let xml = "<EncodingOptions>\n  <SomeOption>val</SomeOption>\n</EncodingOptions>";
        let patched = patch_encoding_xml(xml);
        assert!(patched.contains("<HardwareDecodingCodecs>"));
        assert!(patched.contains("<string>hevc</string>"));
    }
}
