use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::credentials::get_gemini_api_key;
use crate::memory::ConversationStore;

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn http_client() -> Result<&'static reqwest::Client, String> {
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;
    Ok(HTTP_CLIENT.get_or_init(|| client))
}

const GEMINI_MODEL: &str = "gemini-3.8-flash";
const GEMINI_INTERACTIONS_ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/interactions";

/// Gemini's own trained bounding-box format: box_2d = [ymin, xmin, ymax,
/// xmax], each normalized to 0-1000. Annotations are a secondary, optional
/// part of the response -- the primary goal is a good actionable "text"
/// answer, not pixel-perfect pointing.
#[derive(Deserialize, Serialize, Debug)]
struct RawAnnotation {
    #[serde(rename = "type")]
    annotation_type: String,
    box_2d: [f64; 4],
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize, Debug)]
struct RawVlmResponse {
    text: String,
    annotations: Vec<RawAnnotation>,
}

#[derive(Serialize, Debug)]
pub struct Annotation {
    #[serde(rename = "type")]
    pub annotation_type: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct VlmResponse {
    pub text: String,
    pub annotations: Vec<Annotation>,
}

fn annotation_from_box_2d(raw: RawAnnotation) -> Annotation {
    let [ymin, xmin, ymax, xmax] = raw.box_2d;
    let x = (xmin / 1000.0).clamp(0.0, 1.0);
    let y = (ymin / 1000.0).clamp(0.0, 1.0);
    let x2 = (xmax / 1000.0).clamp(0.0, 1.0);
    let y2 = (ymax / 1000.0).clamp(0.0, 1.0);
    Annotation {
        annotation_type: raw.annotation_type,
        x,
        y,
        width: (x2 - x).max(0.0),
        height: (y2 - y).max(0.0),
        label: raw.label,
    }
}

fn response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "text": { "type": "string" },
            "annotations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["ring", "arrow", "box", "underline"]
                        },
                        "box_2d": {
                            "type": "array",
                            "items": { "type": "integer" },
                            "minItems": 4,
                            "maxItems": 4
                        },
                        "label": { "type": "string" }
                    },
                    "required": ["type", "box_2d"]
                }
            }
        },
        "required": ["text", "annotations"]
    })
}

fn build_prompt(query: &str, has_crop: bool, cursor: Option<(f64, f64)>, history: &str) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "You are a screen-side assistant that watches the user's screen and helps them figure \
        out what to do next. Look at the attached screenshot(s) and answer the user's question.\n\n",
    );

    if !history.is_empty() {
        prompt.push_str(&format!(
            "Here is the recent conversation history in this app, for context if the question \
            below is a follow-up (e.g. \"and the second one?\"):\n{history}\n\n"
        ));
        prompt.push_str(
            "This history is only extra context -- it does not make you any less certain about \
            what's on screen right now. If this follow-up question has an identifiable on-screen \
            answer, annotate it just as confidently as you would for a first question; being a \
            follow-up is not by itself a reason to leave \"annotations\" empty. If a previous turn \
            above includes annotations and this follow-up is still about that same on-screen \
            subject, reuse its exact box_2d coordinates verbatim instead of re-deriving new ones \
            -- only compute a new box_2d if the follow-up clearly points at a different location.\n\n",
        );
    }

    if let Some((cx, cy)) = cursor {
        prompt.push_str(&format!(
            "The user's mouse cursor was at approximately x={cx:.3}, y={cy:.3} (as a fraction \
            of the full screen, where (0,0) is the top-left corner and (1,1) is the bottom-right \
            corner) at the moment they asked this question. "
        ));
        if has_crop {
            prompt.push_str(
                "The first image is the full screen for overall context. The second image is a \
                large, zoomed-in crop centered on that cursor position -- it covers a sizeable \
                portion of the screen around the cursor, so use it to read fine detail (small \
                icons, labels, menu items) near where the user is pointing that may not be legible \
                in the full screenshot. ",
            );
        }
        prompt.push_str(
            "If the question refers to \"this\", \"here\", or a vague location without further \
            detail, assume the user means whatever is at or near the cursor position.\n\n",
        );
    }

    prompt.push_str(&format!("User's question: {query}\n\n"));
    prompt.push_str(
        "Respond ONLY with a JSON object matching the provided schema. In \"text\", give a clear, \
        actionable answer. If the user is asking how to do something, do not just describe what \
        is on screen -- tell them exactly what to click or type, where it is located (e.g. \"in \
        the panel on the left\", \"the icon next to the search box at the top of that panel\", \
        \"right-click inside the empty space below the file list\"), and what to expect to happen, \
        referencing the actual labels, icons, menu names, or panel names visible in the \
        screenshot(s). Prefer concrete step-by-step instructions over vague descriptions. In \
        \"annotations\", optionally list a small number of on-screen locations that are directly \
        relevant to the steps you described, each with a \"box_2d\" field: [ymin, xmin, ymax, \
        xmax], each an integer from 0 to 1000, normalized against the full screen (the first \
        image) -- NOT the zoomed crop's own coordinates, even when a crop is provided. Use \"box\" \
        for a region, or \"ring\", \"arrow\", or \"underline\" for a single point. Annotations are \
        a bonus, not the point of the answer -- if you are not confident about a location, leave \
        \"annotations\" as [] rather than guessing.",
    );

    prompt
}

fn describe_error(e: &dyn std::error::Error) -> String {
    let mut parts = vec![e.to_string()];
    let mut source = e.source();
    while let Some(s) = source {
        parts.push(s.to_string());
        source = s.source();
    }
    parts.join(" -> caused by: ")
}

#[tauri::command]
pub async fn analyze_screenshot(
    image_base64: String,
    query: String,
    crop_base64: Option<String>,
    cursor_x_frac: Option<f64>,
    cursor_y_frac: Option<f64>,
    app_id: Option<String>,
    history: tauri::State<'_, ConversationStore>,
) -> Result<VlmResponse, String> {
    let api_key = get_gemini_api_key()
        .map_err(|_| "No Gemini API key configured. Open Settings to add one.".to_string())?;

    let cursor = match (cursor_x_frac, cursor_y_frac) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };

    let history_text = app_id
        .as_deref()
        .map(|id| history.history_text(id))
        .unwrap_or_default();

    let prompt_text = build_prompt(&query, crop_base64.is_some(), cursor, &history_text);
    let mut input = vec![
        json!({ "type": "text", "text": prompt_text }),
        json!({ "type": "image", "data": image_base64, "mime_type": "image/png" }),
    ];
    if let Some(crop) = &crop_base64 {
        input.push(json!({ "type": "image", "data": crop, "mime_type": "image/png" }));
    }

    let body = json!({
        "model": GEMINI_MODEL,
        "input": input,
        "response_format": {
            "type": "text",
            "mime_type": "application/json",
            "schema": response_schema()
        },
        "store": false
    });

    let response = http_client()?
        .post(GEMINI_INTERACTIONS_ENDPOINT)
        .header("x-goog-api-key", &api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network error contacting Gemini: {}", describe_error(&e)))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read Gemini response body: {e}"))?;

    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&response_text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
            .unwrap_or(response_text);
        return Err(format!("Gemini API error ({status}): {message}"));
    }

    let parsed: serde_json::Value = serde_json::from_str(&response_text)
        .map_err(|e| format!("Could not parse Gemini's response as JSON: {e}"))?;

    let steps = parsed["steps"].as_array().ok_or_else(|| {
        "Gemini returned no usable content (unexpected response shape)".to_string()
    })?;

    let output_text = steps
        .iter()
        .find(|step| step["type"] == "model_output")
        .and_then(|step| step["content"][0]["text"].as_str())
        .ok_or_else(|| {
            "Gemini returned no usable content (it may have been blocked by safety filters)"
                .to_string()
        })?;

    let raw: RawVlmResponse = serde_json::from_str(output_text)
        .map_err(|e| format!("Could not parse Gemini's structured answer: {e}"))?;

    println!(
        "[vlm] history_present={} annotations={}",
        !history_text.is_empty(),
        raw.annotations.len()
    );

    if let Some(id) = app_id.as_deref() {
        let annotations_json = serde_json::to_string(&raw.annotations).unwrap_or_else(|_| "[]".to_string());
        history.push_turn(id, query.clone(), raw.text.clone(), annotations_json);
    }

    Ok(VlmResponse {
        text: raw.text,
        annotations: raw.annotations.into_iter().map(annotation_from_box_2d).collect(),
    })
}
