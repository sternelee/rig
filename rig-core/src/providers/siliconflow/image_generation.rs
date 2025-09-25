use crate::providers::siliconflow::{ApiResponse, Client};
use crate::image_generation;
use crate::image_generation::{ImageGenerationError, ImageGenerationRequest};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::Deserialize;
use serde_json::json;

// ================================================================
// SiliconFlow Image Generation API
// ================================================================

/// Stable Diffusion XL model
pub const STABLE_DIFFUSION_XL: &str = "stabilityai/stable-diffusion-xl-base-1.0";
/// FLUX.1-dev model
pub const FLUX_1_DEV: &str = "black-forest-labs/FLUX.1-dev";
/// FLUX.1-schnell model
pub const FLUX_1_SCHNELL: &str = "black-forest-labs/FLUX.1-schnell";

#[derive(Debug, Deserialize)]
pub struct ImageGenerationData {
    pub b64_json: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageGenerationResponse {
    pub created: i32,
    pub data: Vec<ImageGenerationData>,
}

impl TryFrom<ImageGenerationResponse>
    for image_generation::ImageGenerationResponse<ImageGenerationResponse>
{
    type Error = ImageGenerationError;

    fn try_from(value: ImageGenerationResponse) -> Result<Self, Self::Error> {
        let b64_json = value.data[0].b64_json.clone();

        let bytes = BASE64_STANDARD
            .decode(&b64_json)
            .map_err(|e| ImageGenerationError::ResponseError(format!("Failed to decode b64: {}", e)))?;

        Ok(image_generation::ImageGenerationResponse {
            image: bytes,
            response: value,
        })
    }
}

#[derive(Clone)]
pub struct ImageGenerationModel {
    client: Client,
    /// Name of the model (e.g.: stabilityai/stable-diffusion-xl-base-1.0)
    pub model: String,
}

impl ImageGenerationModel {
    pub(crate) fn new(client: Client, model: &str) -> Self {
        Self {
            client,
            model: model.to_string(),
        }
    }
}

impl image_generation::ImageGenerationModel for ImageGenerationModel {
    type Response = ImageGenerationResponse;

    #[cfg_attr(feature = "worker", worker::send)]
    async fn image_generation(
        &self,
        generation_request: ImageGenerationRequest,
    ) -> Result<image_generation::ImageGenerationResponse<Self::Response>, ImageGenerationError>
    {
        let mut request = json!({
            "model": self.model,
            "prompt": generation_request.prompt,
        });

        // Add size parameters for SiliconFlow
        let request_obj = request.as_object_mut().unwrap();
        request_obj.insert("width".to_string(), json!(generation_request.width));
        request_obj.insert("height".to_string(), json!(generation_request.height));
        request_obj.insert("response_format".to_string(), json!("b64_json"));

        // Add any additional parameters
        if let Some(params) = generation_request.additional_params {
            if let Some(obj) = params.as_object() {
                for (key, value) in obj {
                    request_obj.insert(key.clone(), value.clone());
                }
            }
        }

        let response = self
            .client
            .post("/images/generations")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ImageGenerationError::ProviderError(format!(
                "{}: {}",
                response.status(),
                response.text().await?
            )));
        }

        let response_text = response.text().await?;

        match serde_json::from_str::<ApiResponse<ImageGenerationResponse>>(&response_text)? {
            ApiResponse::Ok(response) => response.try_into(),
            ApiResponse::Err(err) => Err(ImageGenerationError::ProviderError(err.message)),
        }
    }
}