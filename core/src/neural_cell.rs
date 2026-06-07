use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ExperimentalNeuralCell {
    layers: Vec<DenseLayer>,
    normalization: Normalization,
}

#[derive(Clone, Debug)]
struct DenseLayer {
    in_features: usize,
    out_features: usize,
    weight: Vec<f32>,
    bias: Vec<f32>,
}

#[derive(Clone, Copy, Debug)]
struct Normalization {
    input_mean: f32,
    input_std: f32,
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
    input_mean: f32,
    input_std: f32,
    output_mean: f32,
    output_std: f32,
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
        let mut values =
            vec![(input_v - self.normalization.input_mean) / self.normalization.input_std];
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

    pub fn process_block(&self, input_v: &[f32], output_v: &mut [f32]) -> Result<()> {
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
        if layers[0].in_features != 1 || layers.last().is_some_and(|layer| layer.out_features != 1)
        {
            bail!("only scalar input/output neural cells are supported");
        }
        let normalization = Normalization {
            input_mean: descriptor.io.normalization.input_mean,
            input_std: nonzero_std(descriptor.io.normalization.input_std, "input_std")?,
            output_mean: descriptor.io.normalization.output_mean,
            output_std: nonzero_std(descriptor.io.normalization.output_std, "output_std")?,
        };
        Ok(Self {
            layers,
            normalization,
        })
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
