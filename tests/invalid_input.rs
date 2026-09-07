use tileforge_draco::{Attribute, AttributeType, EncodeOptions, MeshView, Quantization, encode};

// Each malformed input runs in its own process. A native abort must fail the
// parent assertion, rather than terminating the remainder of the test suite.
#[test]
fn invalid_numeric_inputs_return_argument_errors() {
    if let Ok(case) = std::env::var("TF_DRACO_INVALID_CASE") {
        let mut positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let mut uv = [0.0; 6];
        let mut origin = [0.0; 2];
        let mut range = 1.0;
        let mut quantization = Quantization::Grid { spacing: 1.0 };
        match case.as_str() {
            "nan_position" => positions[0] = f32::NAN,
            "infinite_position" => positions[0] = f32::INFINITY,
            "negative_infinite_position" => positions[0] = f32::NEG_INFINITY,
            "bits_nan_position" => {
                positions[0] = f32::NAN;
                quantization = Quantization::Bits { bits: 14 };
            }
            "nan_attribute" => uv[0] = f32::NAN,
            "infinite_attribute" => uv[0] = f32::INFINITY,
            "nan_origin" => origin[0] = f32::NAN,
            "infinite_origin" => origin[0] = f32::INFINITY,
            "infinite_range" => range = f32::INFINITY,
            _ => panic!("unknown case"),
        }
        let attributes = [
            Attribute::positions(&positions),
            Attribute {
                kind: AttributeType::TexCoord,
                components: 2,
                quantization_bits: 12,
                data: &uv,
                explicit: Some((&origin, range)),
            },
        ];
        let error = encode(
            MeshView {
                attributes: &attributes,
                indices: &[0, 1, 2],
                num_vertices: 3,
            },
            &EncodeOptions {
                position: quantization,
                speed: 0,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, 1);
        assert!(error.message.contains("finite"), "{error}");
        return;
    }
    for case in [
        "nan_position",
        "infinite_position",
        "negative_infinite_position",
        "bits_nan_position",
        "nan_attribute",
        "infinite_attribute",
        "nan_origin",
        "infinite_origin",
        "infinite_range",
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "invalid_numeric_inputs_return_argument_errors",
                "--nocapture",
            ])
            .env("TF_DRACO_INVALID_CASE", case)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
