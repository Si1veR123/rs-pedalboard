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

/// Return a list of configs that support the given sample rate and buffer size.
fn get_compatible_configs(
    configs: impl Iterator<Item = SupportedStreamConfigRange>,
    sample_rate: usize,
    buffer_size: usize
) -> Vec<SupportedStreamConfig> {
    configs.filter_map(|c| c.try_with_sample_rate(cpal::SampleRate(sample_rate as u32)))
        .filter(|c| {
            match c.buffer_size() {
                SupportedBufferSize::Range { min, max } => *min <= buffer_size as u32 && *max >= buffer_size as u32,
                SupportedBufferSize::Unknown => true,
            }
        })
        .collect()
}

pub fn get_input_config_for_device(device: &Device, sample_rate: usize, buffer_size: usize) -> SupportedStreamConfig {
    let mut compatible_configs = get_compatible_configs(
        device.supported_input_configs().expect("Failed to get supported input configs"),
        sample_rate,
        buffer_size
    );

    if compatible_configs.is_empty() {
        let supported_configs_range = device.supported_input_configs()
            .expect("Failed to get supported input configs")
            .collect::<Vec<_>>();
        log::error!("Supported input configs: {:?}", supported_configs_range);
        panic!("No compatible input or output configs found for sample rate={}, buffer size={} Please check your audio devices.", sample_rate, buffer_size);
    }

    // Prioritise configs based on sample format
    compatible_configs.sort_by(|a, b| {
        sample_format_sort_function(&a.sample_format(), &b.sample_format())
    });

    compatible_configs[0].clone()
}

pub fn get_output_config_for_device(device: &Device, sample_rate: usize, buffer_size: usize) -> SupportedStreamConfig {
    let mut compatible_configs = get_compatible_configs(
        device.supported_output_configs().expect("Failed to get supported output configs"),
        sample_rate,
        buffer_size
    );

    if compatible_configs.is_empty() {
        let supported_configs_range = device.supported_output_configs()
            .expect("Failed to get supported output configs")
            .collect::<Vec<_>>();
        log::error!("Supported output configs: {:?}", supported_configs_range);
        panic!("No compatible input or output configs found for sample rate={}, buffer size={} and channels=1. Please check your audio devices.", sample_rate, buffer_size);
    }

    // Prioritise configs based on first channel count, then sample type, for output
    // This ensures stereo output is preferred over mono
    compatible_configs.sort_by(|a, b| {
        if a.channels() != b.channels() {
            b.channels().cmp(&a.channels())
        } else {
            sample_format_sort_function(&a.sample_format(), &b.sample_format())
        }
    });

    compatible_configs[0].clone()
}
