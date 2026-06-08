use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ExperimentalNeuralCell {
    layers: Vec<DenseLayer>,
    normalization: Normalization,
    input_features: usize,
}

#[derive(Clone, Debug)]
pub struct NeuralCellRuntime {
    cell: ExperimentalNeuralCell,
    input_history: Vec<f32>,
    scratch_a: Vec<f32>,
    scratch_b: Vec<f32>,
}

#[derive(Clone, Copy, Debug)]
pub struct CommonCathodeNeuralAdapterParams {
    pub input_gain: f32,
    pub output_scale: f32,
    pub output_bias: f32,
}

#[derive(Clone, Debug)]
pub struct CommonCathodeNeuralAdapter {
    runtime: NeuralCellRuntime,
    params: CommonCathodeNeuralAdapterParams,
    last_plate_ac_v: f32,
    last_output_v: f32,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct CommonCathodeGrayboxStateParams {
    pub drive_gain: f32,
    pub shape: f32,
    pub fast_alpha: f32,
    pub slow_alpha: f32,
    pub drive_bias: f32,
    pub linear: f32,
    pub saturation: f32,
    pub cubic: f32,
    pub fast_feedback: f32,
    pub slow_feedback: f32,
    pub fast_mix: f32,
    pub slow_mix: f32,
    pub output_gain: f32,
    pub output_bias: f32,
}

#[derive(Clone, Debug)]
pub struct CommonCathodeGrayboxStateCell {
    params: CommonCathodeGrayboxStateParams,
    fast_state: f32,
    slow_state: f32,
}

#[derive(Clone, Debug)]
struct DenseLayer {
    in_features: usize,
    out_features: usize,
    weight: Vec<f32>,
    bias: Vec<f32>,
}

#[derive(Clone, Debug)]
struct Normalization {
    input_mean: Vec<f32>,
    input_std: Vec<f32>,
    output_mean: f32,
    output_std: f32,
}

#[derive(Debug, Deserialize)]
struct Descriptor {
    architecture: ArchitectureDescriptor,
    io: IoDescriptor,
    weights: WeightsDescriptor,
}

#[derive(Debug, Deserialize)]
struct ArchitectureDescriptor {
    family: String,
    activation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IoDescriptor {
    normalization: NormalizationDescriptor,
}

#[derive(Debug, Deserialize)]
struct NormalizationDescriptor {
    input_mean: ScalarOrVector,
    input_std: ScalarOrVector,
    output_mean: f32,
    output_std: f32,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ScalarOrVector {
    Scalar(f32),
    Vector(Vec<f32>),
}

#[derive(Debug, Deserialize)]
struct WeightsDescriptor {
    format: String,
    path: String,
    dtype: String,
    endianness: String,
    layout: Vec<LayerDescriptor>,
}

#[derive(Debug, Deserialize)]
struct LayerDescriptor {
    in_features: usize,
    out_features: usize,
}

impl ExperimentalNeuralCell {
    pub fn from_descriptor_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read neural-cell descriptor {}", path.display()))?;
        let descriptor: Descriptor = json5::from_str(&text).with_context(|| {
            format!("failed to parse neural-cell descriptor {}", path.display())
        })?;
        Self::from_descriptor(&descriptor, path.parent().unwrap_or_else(|| Path::new(".")))
    }

    pub fn process_sample(&self, input_v: f32) -> f32 {
        let mut values = vec![0.0; self.input_features];
        values[0] = (input_v - self.normalization.input_mean[0]) / self.normalization.input_std[0];
        for index in 1..self.input_features {
            values[index] = (0.0 - self.normalization.input_mean[index])
                / self.normalization.input_std[index];
        }
        self.process_normalized_features(values)
    }

    pub fn process_features(&self, features: &[f32]) -> Result<f32> {
        if features.len() != self.input_features {
            bail!(
                "neural-cell feature length mismatch: {} != {}",
                features.len(),
                self.input_features
            );
        }
        let mut values = vec![0.0; self.input_features];
        for index in 0..self.input_features {
            values[index] = (features[index] - self.normalization.input_mean[index])
                / self.normalization.input_std[index];
        }
        Ok(self.process_normalized_features(values))
    }

    fn process_normalized_features(&self, mut values: Vec<f32>) -> f32 {
        for (index, layer) in self.layers.iter().enumerate() {
            let mut next = vec![0.0; layer.out_features];
            for out_index in 0..layer.out_features {
                let row = out_index * layer.in_features;
                let mut sum = layer.bias[out_index];
                for in_index in 0..layer.in_features {
                    sum += layer.weight[row + in_index] * values[in_index];
                }
                next[out_index] = if index + 1 == self.layers.len() {
                    sum
                } else {
                    sum.tanh()
                };
            }
            values = next;
        }
        values[0] * self.normalization.output_std + self.normalization.output_mean
    }

    pub fn prepare_runtime(&self) -> NeuralCellRuntime {
        NeuralCellRuntime::new(self.clone())
    }

    pub fn into_runtime(self) -> NeuralCellRuntime {
        NeuralCellRuntime::new(self)
    }

    pub fn process_block(&self, input_v: &[f32], output_v: &mut [f32]) -> Result<()> {
        if input_v.len() != output_v.len() {
            bail!(
                "neural-cell input/output length mismatch: {} != {}",
                input_v.len(),
                output_v.len()
            );
        }
        let mut runtime = self.prepare_runtime();
        for (input, output) in input_v.iter().zip(output_v.iter_mut()) {
            *output = runtime.process_sample(*input);
        }
        Ok(())
    }

    fn from_descriptor(descriptor: &Descriptor, descriptor_dir: &Path) -> Result<Self> {
        if descriptor.architecture.family != "mlp" {
            bail!(
                "unsupported neural-cell architecture '{}'",
                descriptor.architecture.family
            );
        }
        let activation = descriptor
            .architecture
            .activation
            .as_deref()
            .unwrap_or("tanh");
        if activation != "tanh" {
            bail!("unsupported neural-cell activation '{}'", activation);
        }
        if descriptor.weights.format != "greybound-bin-v1" {
            bail!(
                "unsupported neural-cell weight format '{}'",
                descriptor.weights.format
            );
        }
        if descriptor.weights.dtype != "f32" || descriptor.weights.endianness != "little" {
            bail!(
                "unsupported neural-cell weight encoding '{}'/{}",
                descriptor.weights.dtype,
                descriptor.weights.endianness
            );
        }
        let weights_path = resolve_weights_path(descriptor_dir, &descriptor.weights.path);
        let layers = read_layers(&weights_path, &descriptor.weights.layout)?;
        if layers.is_empty() {
            bail!("neural-cell has no layers");
        }
        if layers[0].in_features == 0 || layers.last().is_some_and(|layer| layer.out_features != 1)
        {
            bail!("only causal scalar-output neural cells are supported");
        }
        let normalization = Normalization {
            input_mean: expand_normalization(
                &descriptor.io.normalization.input_mean,
                layers[0].in_features,
                "input_mean",
            )?,
            input_std: expand_normalization(
                &descriptor.io.normalization.input_std,
                layers[0].in_features,
                "input_std",
            )?
            .into_iter()
            .map(|value| nonzero_std(value, "input_std"))
            .collect::<Result<Vec<_>>>()?,
            output_mean: descriptor.io.normalization.output_mean,
            output_std: nonzero_std(descriptor.io.normalization.output_std, "output_std")?,
        };
        Ok(Self {
            input_features: layers[0].in_features,
            layers,
            normalization,
        })
    }
}

impl NeuralCellRuntime {
    pub fn new(cell: ExperimentalNeuralCell) -> Self {
        let max_width = cell
            .layers
            .iter()
            .map(|layer| layer.in_features.max(layer.out_features))
            .max()
            .unwrap_or(1)
            .max(1);
        Self {
            cell,
            input_history: vec![0.0; max_width],
            scratch_a: vec![0.0; max_width],
            scratch_b: vec![0.0; max_width],
        }
    }

    pub fn input_features(&self) -> usize {
        self.cell.input_features
    }

    #[inline]
    pub fn process_sample(&mut self, input_v: f32) -> f32 {
        let input_features = self.cell.input_features;
        if input_features > 1 {
            self.input_history.copy_within(0..input_features - 1, 1);
        }
        self.input_history[0] = input_v;
        for index in 0..input_features {
            self.scratch_a[index] = (self.input_history[index]
                - self.cell.normalization.input_mean[index])
                / self.cell.normalization.input_std[index];
        }
        let mut input_len = input_features;
        for (index, layer) in self.cell.layers.iter().enumerate() {
            debug_assert_eq!(input_len, layer.in_features);
            for out_index in 0..layer.out_features {
                let row = out_index * layer.in_features;
                let mut sum = layer.bias[out_index];
                for in_index in 0..layer.in_features {
                    sum += layer.weight[row + in_index] * self.scratch_a[in_index];
                }
                self.scratch_b[out_index] = if index + 1 == self.cell.layers.len() {
                    sum
                } else {
                    sum.tanh()
                };
            }
            self.scratch_a[..layer.out_features]
                .copy_from_slice(&self.scratch_b[..layer.out_features]);
            input_len = layer.out_features;
        }
        self.scratch_a[0] * self.cell.normalization.output_std + self.cell.normalization.output_mean
    }

    pub fn process_features(&mut self, features: &[f32]) -> Result<f32> {
        if features.len() != self.cell.input_features {
            bail!(
                "neural-cell feature length mismatch: {} != {}",
                features.len(),
                self.cell.input_features
            );
        }
        for (index, feature) in features.iter().enumerate() {
            self.scratch_a[index] = (*feature - self.cell.normalization.input_mean[index])
                / self.cell.normalization.input_std[index];
        }
        let mut input_len = self.cell.input_features;
        for (index, layer) in self.cell.layers.iter().enumerate() {
            debug_assert_eq!(input_len, layer.in_features);
            for out_index in 0..layer.out_features {
                let row = out_index * layer.in_features;
                let mut sum = layer.bias[out_index];
                for in_index in 0..layer.in_features {
                    sum += layer.weight[row + in_index] * self.scratch_a[in_index];
                }
                self.scratch_b[out_index] = if index + 1 == self.cell.layers.len() {
                    sum
                } else {
                    sum.tanh()
                };
            }
            self.scratch_a[..layer.out_features]
                .copy_from_slice(&self.scratch_b[..layer.out_features]);
            input_len = layer.out_features;
        }
        Ok(self.scratch_a[0] * self.cell.normalization.output_std
            + self.cell.normalization.output_mean)
    }

    pub fn process_block(&mut self, input_v: &[f32], output_v: &mut [f32]) -> Result<()> {
        if input_v.len() != output_v.len() {
            bail!(
                "neural-cell input/output length mismatch: {} != {}",
                input_v.len(),
                output_v.len()
            );
        }
        for (input, output) in input_v.iter().zip(output_v.iter_mut()) {
            *output = self.process_sample(*input);
        }
        Ok(())
    }
}

impl CommonCathodeNeuralAdapter {
    pub fn new(runtime: NeuralCellRuntime, params: CommonCathodeNeuralAdapterParams) -> Self {
        Self {
            runtime,
            params,
            last_plate_ac_v: 0.0,
            last_output_v: 0.0,
        }
    }

    pub fn from_cell(
        cell: ExperimentalNeuralCell,
        params: CommonCathodeNeuralAdapterParams,
    ) -> Self {
        Self::new(cell.into_runtime(), params)
    }

    #[inline]
    pub fn process_sample(&mut self, input_v: f32) -> f32 {
        let plate_ac_v = self
            .runtime
            .process_sample(input_v * self.params.input_gain);
        let output_v = self.params.output_bias - plate_ac_v * self.params.output_scale;
        self.last_plate_ac_v = plate_ac_v;
        self.last_output_v = output_v;
        output_v
    }

    pub fn process_block(&mut self, input_v: &[f32], output_v: &mut [f32]) -> Result<()> {
        if input_v.len() != output_v.len() {
            bail!(
                "common-cathode neural adapter input/output length mismatch: {} != {}",
                input_v.len(),
                output_v.len()
            );
        }
        for (input, output) in input_v.iter().zip(output_v.iter_mut()) {
            *output = self.process_sample(*input);
        }
        Ok(())
    }

    pub fn last_plate_ac_v(&self) -> f32 {
        self.last_plate_ac_v
    }

    pub fn last_output_v(&self) -> f32 {
        self.last_output_v
    }
}

impl CommonCathodeGrayboxStateCell {
    pub fn new(params: CommonCathodeGrayboxStateParams) -> Result<Self> {
        params.validate()?;
        Ok(Self {
            params,
            fast_state: 0.0,
            slow_state: 0.0,
        })
    }

    pub fn from_config_path(path: impl AsRef<Path>) -> Result<Self> {
        #[derive(Deserialize)]
        struct Config {
            parameters: CommonCathodeGrayboxStateParams,
        }

        let path = path.as_ref();
        if let Some(name) = path.to_str() {
            if matches!(
                name,
                "accepted" | "accepted-live" | "builtin" | "common-cathode-12ax7-graybox-state-v0"
            ) {
                return Self::new(CommonCathodeGrayboxStateParams::common_cathode_12ax7_v0());
            }
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read gray-box config {}", path.display()))?;
        let config: Config = json5::from_str(&text)
            .with_context(|| format!("failed to parse gray-box config {}", path.display()))?;
        Self::new(config.parameters)
    }

    pub fn reset(&mut self) {
        self.fast_state = 0.0;
        self.slow_state = 0.0;
    }

    #[inline]
    pub fn process_sample(&mut self, input_v: f32) -> f32 {
        let drive = self.params.drive_gain * input_v
            - self.params.fast_feedback * self.fast_state
            - self.params.slow_feedback * self.slow_state
            + self.params.drive_bias;
        let instant = self.params.linear * drive
            + self.params.saturation * (self.params.shape * drive).tanh()
            + self.params.cubic * drive * drive * drive;
        self.fast_state += self.params.fast_alpha * (instant - self.fast_state);
        self.slow_state += self.params.slow_alpha * (self.fast_state - self.slow_state);
        self.params.output_gain
            * (instant
                + self.params.fast_mix * self.fast_state
                + self.params.slow_mix * self.slow_state)
            + self.params.output_bias
    }

    pub fn process_block(&mut self, input_v: &[f32], output_v: &mut [f32]) -> Result<()> {
        if input_v.len() != output_v.len() {
            bail!(
                "common-cathode gray-box input/output length mismatch: {} != {}",
                input_v.len(),
                output_v.len()
            );
        }
        for (input, output) in input_v.iter().zip(output_v.iter_mut()) {
            *output = self.process_sample(*input);
        }
        Ok(())
    }

    pub fn params(&self) -> CommonCathodeGrayboxStateParams {
        self.params
    }
}

impl CommonCathodeGrayboxStateParams {
    pub fn common_cathode_12ax7_v0() -> Self {
        Self {
            drive_gain: 2.1871016,
            shape: 1.2695892,
            fast_alpha: 0.13404444,
            slow_alpha: 0.0025320612,
            drive_bias: -0.0008933089,
            linear: -5.0462627,
            saturation: -1.0460438,
            cubic: -0.23338625,
            fast_feedback: -0.008940127,
            slow_feedback: -0.05190596,
            fast_mix: 0.05544361,
            slow_mix: -0.042792317,
            output_gain: 1.0831846,
            output_bias: -0.008414772,
        }
    }

    fn validate(&self) -> Result<()> {
        let values = [
            self.drive_gain,
            self.shape,
            self.fast_alpha,
            self.slow_alpha,
            self.drive_bias,
            self.linear,
            self.saturation,
            self.cubic,
            self.fast_feedback,
            self.slow_feedback,
            self.fast_mix,
            self.slow_mix,
            self.output_gain,
            self.output_bias,
        ];
        if !values.iter().all(|value| value.is_finite()) {
            bail!("gray-box parameters must be finite");
        }
        if self.shape <= 0.0 {
            bail!("gray-box shape must be positive");
        }
        if !(0.0..=1.0).contains(&self.fast_alpha) || !(0.0..=1.0).contains(&self.slow_alpha) {
            bail!("gray-box state coefficients must be in [0, 1]");
        }
        Ok(())
    }
}

fn read_layers(path: &Path, layout: &[LayerDescriptor]) -> Result<Vec<DenseLayer>> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to open neural-cell weights {}", path.display()))?;
    let mut layers = Vec::with_capacity(layout.len());
    for layer in layout {
        let weight_count = layer
            .in_features
            .checked_mul(layer.out_features)
            .context("neural-cell layer dimensions overflow")?;
        let weight = read_f32_vector(&mut file, weight_count)?;
        let bias = read_f32_vector(&mut file, layer.out_features)?;
        layers.push(DenseLayer {
            in_features: layer.in_features,
            out_features: layer.out_features,
            weight,
            bias,
        });
    }
    Ok(layers)
}

fn read_f32_vector(file: &mut fs::File, expected_count: usize) -> Result<Vec<f32>> {
    let mut count_bytes = [0u8; 4];
    file.read_exact(&mut count_bytes)
        .context("failed to read neural-cell vector length")?;
    let count = u32::from_le_bytes(count_bytes) as usize;
    if count != expected_count {
        bail!(
            "neural-cell vector has {} values, expected {}",
            count,
            expected_count
        );
    }
    let mut bytes = vec![0u8; count * 4];
    file.read_exact(&mut bytes)
        .context("failed to read neural-cell vector data")?;
    let values = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    Ok(values)
}

fn resolve_weights_path(descriptor_dir: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        descriptor_dir.join(path)
    }
}

fn nonzero_std(value: f32, name: &str) -> Result<f32> {
    if value.abs() <= f32::EPSILON {
        bail!("neural-cell normalization {} must be non-zero", name);
    }
    Ok(value)
}

fn expand_normalization(
    value: &ScalarOrVector,
    input_features: usize,
    name: &str,
) -> Result<Vec<f32>> {
    match value {
        ScalarOrVector::Scalar(item) => Ok(vec![*item; input_features]),
        ScalarOrVector::Vector(items) => {
            if items.len() != input_features {
                bail!(
                    "neural-cell normalization {} has {} values, expected {}",
                    name,
                    items.len(),
                    input_features
                );
            }
            Ok(items.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_mlp_descriptor_and_processes_sample() {
        let dir = test_dir("loads_mlp_descriptor_and_processes_sample");
        fs::create_dir_all(&dir).unwrap();
        write_test_weights(
            &dir.join("weights.greybound.bin"),
            &[(&[1.0_f32][..], &[0.0_f32][..])],
        );
        fs::write(
            dir.join("model.greybound.json"),
            r#"{
              architecture: { family: "mlp", activation: "tanh" },
              io: {
                normalization: {
                  input_mean: 1.0,
                  input_std: 2.0,
                  output_mean: 10.0,
                  output_std: 4.0,
                },
              },
              weights: {
                format: "greybound-bin-v1",
                path: "weights.greybound.bin",
                dtype: "f32",
                endianness: "little",
                layout: [{ in_features: 1, out_features: 1 }],
              },
            }"#,
        )
        .unwrap();

        let cell =
            ExperimentalNeuralCell::from_descriptor_path(dir.join("model.greybound.json")).unwrap();

        assert_eq!(cell.process_sample(1.0), 10.0);
        assert_eq!(cell.process_sample(3.0), 14.0);
        let mut output = [0.0, 0.0];
        cell.process_block(&[1.0, 3.0], &mut output).unwrap();
        assert_eq!(output, [10.0, 14.0]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn runtime_matches_descriptor_path_for_multilayer_cell() {
        let dir = test_dir("runtime_matches_descriptor_path_for_multilayer_cell");
        fs::create_dir_all(&dir).unwrap();
        write_test_weights(
            &dir.join("weights.greybound.bin"),
            &[
                (&[0.5_f32, -0.25_f32][..], &[0.1_f32, -0.2_f32][..]),
                (&[0.75_f32, -1.5_f32][..], &[0.05_f32][..]),
            ],
        );
        fs::write(
            dir.join("model.greybound.json"),
            r#"{
              architecture: { family: "mlp", activation: "tanh" },
              io: {
                normalization: {
                  input_mean: 0.1,
                  input_std: 0.4,
                  output_mean: -0.2,
                  output_std: 1.7,
                },
              },
              weights: {
                format: "greybound-bin-v1",
                path: "weights.greybound.bin",
                dtype: "f32",
                endianness: "little",
                layout: [
                  { in_features: 1, out_features: 2 },
                  { in_features: 2, out_features: 1 },
                ],
              },
            }"#,
        )
        .unwrap();

        let cell =
            ExperimentalNeuralCell::from_descriptor_path(dir.join("model.greybound.json")).unwrap();
        let mut runtime = cell.prepare_runtime();
        for input in [-0.8, -0.1, 0.0, 0.35, 1.2] {
            assert_eq!(runtime.process_sample(input), cell.process_sample(input));
        }
        let mut runtime_output = [0.0; 5];
        runtime
            .process_block(&[-0.8, -0.1, 0.0, 0.35, 1.2], &mut runtime_output)
            .unwrap();
        let mut reference_output = [0.0; 5];
        cell.process_block(&[-0.8, -0.1, 0.0, 0.35, 1.2], &mut reference_output)
            .unwrap();
        assert_eq!(runtime_output, reference_output);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn runtime_uses_causal_input_history() {
        let dir = test_dir("runtime_uses_causal_input_history");
        fs::create_dir_all(&dir).unwrap();
        write_test_weights(
            &dir.join("weights.greybound.bin"),
            &[(&[1.0_f32, 2.0_f32][..], &[0.0_f32][..])],
        );
        fs::write(
            dir.join("model.greybound.json"),
            r#"{
              architecture: { family: "mlp", activation: "tanh" },
              io: {
                normalization: {
                  input_mean: 0.0,
                  input_std: 1.0,
                  output_mean: 0.0,
                  output_std: 1.0,
                },
              },
              weights: {
                format: "greybound-bin-v1",
                path: "weights.greybound.bin",
                dtype: "f32",
                endianness: "little",
                layout: [{ in_features: 2, out_features: 1 }],
              },
            }"#,
        )
        .unwrap();

        let cell =
            ExperimentalNeuralCell::from_descriptor_path(dir.join("model.greybound.json")).unwrap();
        let mut runtime = cell.prepare_runtime();

        assert_eq!(runtime.process_sample(0.5), 0.5);
        assert_eq!(runtime.process_sample(0.25), 1.25);
        let mut output = [0.0, 0.0];
        cell.process_block(&[0.5, 0.25], &mut output).unwrap();
        assert_eq!(output, [0.5, 1.25]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn loads_vector_normalization_and_processes_features() {
        let dir = test_dir("loads_vector_normalization_and_processes_features");
        fs::create_dir_all(&dir).unwrap();
        write_test_weights(
            &dir.join("weights.greybound.bin"),
            &[(&[1.0_f32, 2.0_f32][..], &[0.5_f32][..])],
        );
        fs::write(
            dir.join("model.greybound.json"),
            r#"{
              architecture: { family: "mlp", activation: "tanh" },
              io: {
                normalization: {
                  input_mean: [1.0, 10.0],
                  input_std: [2.0, 5.0],
                  output_mean: -1.0,
                  output_std: 3.0,
                },
              },
              weights: {
                format: "greybound-bin-v1",
                path: "weights.greybound.bin",
                dtype: "f32",
                endianness: "little",
                layout: [{ in_features: 2, out_features: 1 }],
              },
            }"#,
        )
        .unwrap();

        let cell =
            ExperimentalNeuralCell::from_descriptor_path(dir.join("model.greybound.json")).unwrap();
        let output = cell.process_features(&[3.0, 15.0]).unwrap();

        assert_eq!(output, 9.5);
        let mut runtime = cell.prepare_runtime();
        assert_eq!(runtime.process_features(&[3.0, 15.0]).unwrap(), 9.5);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn common_cathode_adapter_maps_plate_ac_to_stage_output() {
        let dir = test_dir("common_cathode_adapter_maps_plate_ac_to_stage_output");
        fs::create_dir_all(&dir).unwrap();
        write_test_weights(
            &dir.join("weights.greybound.bin"),
            &[(&[2.0_f32][..], &[0.5_f32][..])],
        );
        fs::write(
            dir.join("model.greybound.json"),
            r#"{
              architecture: { family: "mlp", activation: "tanh" },
              io: {
                normalization: {
                  input_mean: 0.0,
                  input_std: 1.0,
                  output_mean: 0.0,
                  output_std: 1.0,
                },
              },
              weights: {
                format: "greybound-bin-v1",
                path: "weights.greybound.bin",
                dtype: "f32",
                endianness: "little",
                layout: [{ in_features: 1, out_features: 1 }],
              },
            }"#,
        )
        .unwrap();

        let cell =
            ExperimentalNeuralCell::from_descriptor_path(dir.join("model.greybound.json")).unwrap();
        let mut adapter = CommonCathodeNeuralAdapter::from_cell(
            cell,
            CommonCathodeNeuralAdapterParams {
                input_gain: 3.0,
                output_scale: 0.25,
                output_bias: 10.0,
            },
        );

        let output = adapter.process_sample(0.5);
        assert_eq!(adapter.last_plate_ac_v(), 3.5);
        assert_eq!(output, 9.125);
        assert_eq!(adapter.last_output_v(), 9.125);

        let mut block_output = [0.0, 0.0];
        adapter
            .process_block(&[0.0, 1.0], &mut block_output)
            .unwrap();
        assert_eq!(block_output, [9.875, 8.375]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn graybox_state_cell_processes_stateful_sequence() {
        let mut cell = CommonCathodeGrayboxStateCell::new(CommonCathodeGrayboxStateParams {
            drive_gain: 1.0,
            shape: 1.0,
            fast_alpha: 0.5,
            slow_alpha: 0.25,
            drive_bias: 0.0,
            linear: 1.0,
            saturation: 0.0,
            cubic: 0.0,
            fast_feedback: 0.0,
            slow_feedback: 0.0,
            fast_mix: 1.0,
            slow_mix: 1.0,
            output_gain: 1.0,
            output_bias: 0.0,
        })
        .unwrap();

        let first = cell.process_sample(1.0);
        let second = cell.process_sample(0.0);

        assert!((first - 1.625).abs() < 1.0e-6);
        assert!((second - 0.40625).abs() < 1.0e-6);
    }

    #[test]
    fn graybox_state_cell_rejects_invalid_coefficients() {
        let error = CommonCathodeGrayboxStateCell::new(CommonCathodeGrayboxStateParams {
            drive_gain: 1.0,
            shape: 1.0,
            fast_alpha: 1.2,
            slow_alpha: 0.25,
            drive_bias: 0.0,
            linear: 1.0,
            saturation: 0.0,
            cubic: 0.0,
            fast_feedback: 0.0,
            slow_feedback: 0.0,
            fast_mix: 0.0,
            slow_mix: 0.0,
            output_gain: 1.0,
            output_bias: 0.0,
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("state coefficients"));
    }

    #[test]
    fn graybox_state_cell_loads_accepted_builtin() {
        let mut cell = CommonCathodeGrayboxStateCell::from_config_path("accepted").unwrap();
        let output = cell.process_sample(0.01);

        assert!(output.is_finite());
        assert_eq!(
            cell.params().drive_gain,
            CommonCathodeGrayboxStateParams::common_cathode_12ax7_v0().drive_gain
        );
    }

    #[test]
    fn rejects_wrong_vector_size() {
        let dir = test_dir("rejects_wrong_vector_size");
        fs::create_dir_all(&dir).unwrap();
        let mut file = fs::File::create(dir.join("weights.greybound.bin")).unwrap();
        file.write_all(&2u32.to_le_bytes()).unwrap();
        file.write_all(&0.0f32.to_le_bytes()).unwrap();
        file.write_all(&0.0f32.to_le_bytes()).unwrap();
        file.write_all(&1u32.to_le_bytes()).unwrap();
        file.write_all(&0.0f32.to_le_bytes()).unwrap();
        fs::write(
            dir.join("model.greybound.json"),
            r#"{
              architecture: { family: "mlp", activation: "tanh" },
              io: { normalization: { input_mean: 0.0, input_std: 1.0, output_mean: 0.0, output_std: 1.0 } },
              weights: {
                format: "greybound-bin-v1",
                path: "weights.greybound.bin",
                dtype: "f32",
                endianness: "little",
                layout: [{ in_features: 1, out_features: 1 }],
              },
            }"#,
        )
        .unwrap();

        let error = ExperimentalNeuralCell::from_descriptor_path(dir.join("model.greybound.json"))
            .unwrap_err()
            .to_string();

        assert!(error.contains("expected 1"));
        let _ = fs::remove_dir_all(dir);
    }

    fn write_test_weights(path: &Path, layers: &[(&[f32], &[f32])]) {
        let mut file = fs::File::create(path).unwrap();
        for (weight, bias) in layers {
            write_vector(&mut file, weight);
            write_vector(&mut file, bias);
        }
    }

    fn write_vector(file: &mut fs::File, values: &[f32]) {
        file.write_all(&(values.len() as u32).to_le_bytes())
            .unwrap();
        for value in values {
            file.write_all(&value.to_le_bytes()).unwrap();
        }
    }

    fn test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("greybound_{}_{}", name, unique))
    }
}
