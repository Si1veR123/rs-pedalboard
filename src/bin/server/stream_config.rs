use cpal::{traits::DeviceTrait, Device, SampleFormat, SupportedBufferSize, SupportedStreamConfig, SupportedStreamConfigRange};

const SAMPLE_FORMAT_SORT_ORDER: [SampleFormat; 10] = [
    SampleFormat::F32,
    SampleFormat::F64,
    SampleFormat::I32,
    SampleFormat::U32,
    SampleFormat::I16,
    SampleFormat::U16,
    SampleFormat::I8,
    SampleFormat::U8,
    SampleFormat::I64,
    SampleFormat::U64
];

fn sample_format_sort_function(a: &SampleFormat, b: &SampleFormat) -> std::cmp::Ordering {
    let a_index = SAMPLE_FORMAT_SORT_ORDER.iter().position(|&x| x == *a).unwrap_or(usize::MAX);
    let b_index = SAMPLE_FORMAT_SORT_ORDER.iter().position(|&x| x == *b).unwrap_or(usize::MAX);
    a_index.cmp(&b_index)
}

/// Return a list of configs that support the buffer size.
fn get_compatible_buffer_size_configs(
    configs: impl Iterator<Item = SupportedStreamConfigRange>,
    buffer_size: usize
) -> Vec<SupportedStreamConfigRange> {
    configs.filter(|c| {
            match c.buffer_size() {
                SupportedBufferSize::Range { min, max } => *min <= buffer_size as u32 && *max >= buffer_size as u32,
                SupportedBufferSize::Unknown => true,
            }
        })
        .collect()
}

/// Find config matching the buffer size, and return a list of compatible configs sorted by channel count (prefer min) then sample format preference.
pub fn get_input_config_candidates(device: &Device, buffer_size: usize) -> Vec<SupportedStreamConfigRange> {
    let input_supported_configs = device.supported_input_configs()
        .expect("Failed to get supported input configs")
        .collect::<Vec<_>>();

    tracing::debug!("Supported input configs: {:?}", input_supported_configs);

    let mut buffer_size_compatible_configs = get_compatible_buffer_size_configs(
        input_supported_configs.iter().cloned(),
        buffer_size
    );

    if buffer_size_compatible_configs.is_empty() {
        panic!("No compatible input configs found for buffer size={}", buffer_size);
    }

    // Prioritise configs based on channels (prefer min) then sample format
    buffer_size_compatible_configs.sort_by(|a, b| {
        if a.channels() != b.channels() {
            a.channels().cmp(&b.channels())
        } else {
            sample_format_sort_function(&a.sample_format(), &b.sample_format())
        }
    });

    tracing::debug!("Sorted compatible buffer size input configs: {:?}", buffer_size_compatible_configs);

    buffer_size_compatible_configs
}

/// Find output configs matching the buffer size, and return a list of compatible configs sorted by channel count (prefer max) then sample format preference.
pub fn get_output_config_candidates(device: &Device, buffer_size: usize) -> Vec<SupportedStreamConfigRange> {
    let supported_output_configs = device.supported_output_configs()
        .expect("Failed to get supported output configs")
        .collect::<Vec<_>>();

    tracing::debug!("Supported output configs: {:?}", supported_output_configs);

    let mut buffer_size_compatible_configs = get_compatible_buffer_size_configs(
        supported_output_configs.iter().cloned(),
        buffer_size
    );

    if buffer_size_compatible_configs.is_empty() {
        panic!("No compatible output configs found for buffer size={}", buffer_size);
    }

    // Prioritise configs based on first channel count (prefer stereo then maximum) then sample format
    buffer_size_compatible_configs.sort_by(|a, b| {
        if a.channels() != b.channels() {
            b.channels().cmp(&a.channels())
        } else {
            sample_format_sort_function(&a.sample_format(), &b.sample_format())
        }
    });

    tracing::debug!("Sorted compatible buffer size output configs: {:?}", buffer_size_compatible_configs);

    buffer_size_compatible_configs
}

// Returns a list of compatible input and output configs that support the same sample rate
// in order of format and channel preference
pub fn get_compatible_configs(
    input: &Device,
    output: &Device,
    preferred_sample_rate: Option<u32>,
    buffer_size: usize,
) -> (Vec<SupportedStreamConfig>, Vec<SupportedStreamConfig>) {
    tracing::debug!("Input device default config: {:?}", input.default_input_config());
    tracing::debug!("Output device default config: {:?}", output.default_output_config());

    let input_configs = get_input_config_candidates(input, buffer_size);
    let output_configs = get_output_config_candidates(output, buffer_size);

    // Collect all supported sample rates for input and output
    let input_rates: Vec<u32> = input_configs.iter()
        .flat_map(|cfg| cfg.min_sample_rate().0..=cfg.max_sample_rate().0)
        .collect();

    let output_rates: Vec<u32> = output_configs.iter()
        .flat_map(|cfg| cfg.min_sample_rate().0..=cfg.max_sample_rate().0)
        .collect();

    let mut common_rates: Vec<u32> = input_rates
        .into_iter()
        .filter(|rate| output_rates.contains(rate))
        .collect();

    if common_rates.is_empty() {
        panic!("No common sample rates between input and output devices.");
    }

    // Prefer the preferred_sample_rate, otherwise pick the highest common rate
    common_rates.sort_unstable();

    let max_sample_rate = *common_rates.last().unwrap();
    let chosen_rate = if let Some(preferred_sample_rate) = preferred_sample_rate {
        if common_rates.contains(&preferred_sample_rate) {
            tracing::info!("Using preferred sample rate: {}", preferred_sample_rate);
            preferred_sample_rate
        } else {
            tracing::warn!("Preferred sample rate {} not available, using max sample rate: {}", preferred_sample_rate, max_sample_rate);
            max_sample_rate
        }
    } else {
        tracing::info!("No preferred sample rate specified, using max sample rate: {}", max_sample_rate);
        max_sample_rate
    };

    // Find the all configs that support the chosen sample rate
    let mut input_configs_with_sr = Vec::new();
    for input_config_range in input_configs {
        if let Some(config) = input_config_range.try_with_sample_rate(cpal::SampleRate(chosen_rate)) {
            input_configs_with_sr.push(config);
        }
    }

    let mut output_configs_with_sr = Vec::new();
    for output_config_range in output_configs {
        if let Some(config) = output_config_range.try_with_sample_rate(cpal::SampleRate(chosen_rate)) {
            output_configs_with_sr.push(config);
        }
    }

    tracing::info!("Valid configs in order of preference:");
    tracing::info!("Input configs: {:?}", input_configs_with_sr);
    tracing::info!("Output configs: {:?}", output_configs_with_sr);

    (input_configs_with_sr, output_configs_with_sr)
}