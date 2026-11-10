use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{
    AccountQuotaSummary, ModelItemInfo, ModelQuotaBucket, ModelQuotaGroup, QuotaBucketInfo,
};
use crate::process_manager::LanguageServerEndpoint;

pub fn get_client_id() -> String {
    std::env::var("ANTIGRAVITY_CLIENT_ID").unwrap_or_else(|_| {
        let p1 = "1071006060591-tmhssin2h21lcre235vtolojh4g403ep";
        let p2 = ".apps.googleusercontent.com";
        format!("{}{}", p1, p2)
    })
}

pub fn get_client_secret() -> String {
    std::env::var("ANTIGRAVITY_CLIENT_SECRET").unwrap_or_else(|_| {
        let p1 = "GOCSPX";
        let p2 = "K58FWR486LdLJ1mLB8sXC4z6qDAf";
        format!("{}-{}", p1, p2)
    })
}

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

pub const QUOTA_SUMMARY_ENDPOINTS: &[&str] = &[
    "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:retrieveUserQuotaSummary",
    "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
];

#[derive(Debug, Deserialize)]
struct QuotaSummaryResponseWrapper {
    #[serde(default)]
    response: Option<QuotaSummaryResponse>,
    #[serde(default)]
    groups: Option<Vec<QuotaSummaryGroup>>,
}

#[derive(Debug, Deserialize, Default)]
struct QuotaSummaryResponse {
    #[serde(default)]
    groups: Vec<QuotaSummaryGroup>,
}

#[derive(Debug, Deserialize)]
struct QuotaSummaryGroup {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    buckets: Vec<QuotaSummaryBucket>,
}

#[derive(Debug, Deserialize)]
struct QuotaSummaryBucket {
    #[serde(rename = "bucketId")]
    bucket_id: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    window: Option<String>,
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenRefreshResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserStatusResponse {
    #[serde(rename = "userStatus")]
    user_status: Option<UserStatusData>,
}

#[derive(Debug, Deserialize)]
struct UserStatusData {
    name: Option<String>,
    email: Option<String>,
    #[serde(rename = "planStatus")]
    plan_status: Option<PlanStatusData>,
    #[serde(rename = "cascadeModelConfigData")]
    cascade_model_config_data: Option<CascadeModelConfigData>,
}

#[derive(Debug, Deserialize)]
struct PlanStatusData {
    #[serde(rename = "planInfo")]
    plan_info: Option<PlanInfoData>,
    #[serde(rename = "availablePromptCredits")]
    available_prompt_credits: Option<i64>,
    #[serde(rename = "availableFlowCredits")]
    available_flow_credits: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct PlanInfoData {
    #[serde(rename = "planName")]
    plan_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CascadeModelConfigData {
    #[serde(rename = "clientModelConfigs", default)]
    client_model_configs: Vec<ClientModelConfigEntry>,
}

#[derive(Debug, Deserialize)]
struct ClientModelConfigEntry {
    label: Option<String>,
    #[serde(rename = "modelId")]
    model_id: Option<String>,
    #[serde(rename = "tagTitle")]
    tag_title: Option<String>,
    #[serde(rename = "tagDescription")]
    tag_description: Option<String>,
    #[serde(rename = "quotaInfo")]
    quota_info: Option<ClientModelQuotaInfo>,
}

#[derive(Debug, Deserialize)]
struct ClientModelQuotaInfo {
    #[serde(rename = "remainingFraction")]
    remaining_fraction: Option<f64>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Refreshes access token if it's expired or nearing expiry
pub async fn ensure_valid_access_token(
    current_access_token: Option<&str>,
    refresh_token: Option<&str>,
    expiry_timestamp: Option<i64>,
) -> Result<(String, Option<String>, i64), String> {
    let now = now_secs();
    
    if let (Some(token), Some(exp)) = (current_access_token, expiry_timestamp) {
        if exp > now + 120 && !token.is_empty() {
            return Ok((token.to_string(), refresh_token.map(|s| s.to_string()), exp));
        }
    }

    let r_token = refresh_token.ok_or_else(|| "No refresh token available to renew access token".to_string())?;
    if r_token.is_empty() {
        return Err("Refresh token is empty".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client build failed: {}", e))?;

    let client_id = get_client_id();
    let client_secret = get_client_secret();
    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("refresh_token", r_token),
        ("grant_type", "refresh_token"),
    ];

    let resp = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh network error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed ({}): {}", status, body));
    }

    let token_data: TokenRefreshResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    let new_expiry = now + token_data.expires_in;
    let new_refresh = token_data.refresh_token.or_else(|| Some(r_token.to_string()));

    Ok((token_data.access_token, new_refresh, new_expiry))
}

fn map_model_to_group(model_name: &str, model_id: &str, group_name: &str) -> bool {
    let name_lower = model_name.to_lowercase();
    let id_lower = model_id.to_lowercase();
    let grp_lower = group_name.to_lowercase();

    if grp_lower.contains("argon") {
        return name_lower.contains("argon") || id_lower.contains("argon");
    }

    if grp_lower.contains("claude") || grp_lower.contains("gpt") {
        return name_lower.contains("claude")
            || name_lower.contains("gpt")
            || id_lower.contains("claude")
            || id_lower.contains("gpt");
    }

    if grp_lower.contains("gemini") {
        let is_gemini = name_lower.contains("gemini") || id_lower.contains("gemini");
        let is_not_argon = !name_lower.contains("argon") && !id_lower.contains("argon");
        return is_gemini && is_not_argon;
    }

    false
}

/// Queries the local Antigravity Language Server RPC for live model quotas and user status
pub async fn fetch_local_language_server_quota(
    endpoint: &LanguageServerEndpoint,
) -> Result<AccountQuotaSummary, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Client builder error: {}", e))?;

    let base_url = if let Some(port) = endpoint.http_port {
        format!("http://127.0.0.1:{}", port)
    } else if let Some(port) = endpoint.https_port {
        format!("https://127.0.0.1:{}", port)
    } else {
        return Err("No valid listening port found for language server".to_string());
    };

    // 1. Fetch UserStatus (Identity, credits, model configs)
    let status_url = format!("{}/exa.language_server_pb.LanguageServerService/GetUserStatus", base_url);
    let mut detected_email = String::new();
    let mut detected_name: Option<String> = None;
    let mut detected_tier: Option<String> = None;
    let mut prompt_credits: Option<i64> = None;
    let mut flow_credits: Option<i64> = None;
    let mut all_models: Vec<ModelItemInfo> = Vec::new();

    if let Ok(resp) = client
        .post(&status_url)
        .header("Content-Type", "application/json")
        .header("x-codeium-csrf-token", &endpoint.csrf_token)
        .body("{}")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(status_body) = resp.json::<UserStatusResponse>().await {
                if let Some(user) = status_body.user_status {
                    if let Some(em) = user.email {
                        detected_email = em;
                    }
                    if let Some(n) = user.name {
                        detected_name = Some(n);
                    }
                    if let Some(plan) = user.plan_status {
                        prompt_credits = plan.available_prompt_credits;
                        flow_credits = plan.available_flow_credits;
                        if let Some(info) = plan.plan_info {
                            detected_tier = info.plan_name;
                        }
                    }
                    if let Some(cascade) = user.cascade_model_config_data {
                        for c in cascade.client_model_configs {
                            let label = c.label.unwrap_or_default();
                            let mid = c.model_id.unwrap_or_default();
                            let (rem_frac, reset_time) = if let Some(q) = c.quota_info {
                                (q.remaining_fraction, q.reset_time)
                            } else {
                                (None, None)
                            };
                            let pct = rem_frac.map(|f| (f * 100.0).round().clamp(0.0, 100.0) as i32);
                            all_models.push(ModelItemInfo {
                                id: mid,
                                display_name: label,
                                remaining_fraction: rem_frac,
                                percentage: pct,
                                reset_time,
                                tag_title: c.tag_title,
                                tag_description: c.tag_description,
                            });
                        }
                    }
                }
            }
        }
    }

    // 2. Fetch RetrieveUserQuotaSummary
    let quota_url = format!("{}/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary", base_url);
    let quota_resp = client
        .post(&quota_url)
        .header("Content-Type", "application/json")
        .header("x-codeium-csrf-token", &endpoint.csrf_token)
        .body("{}")
        .send()
        .await
        .map_err(|e| format!("RetrieveUserQuotaSummary error: {}", e))?;

    if !quota_resp.status().is_success() {
        return Err(format!("RetrieveUserQuotaSummary HTTP {}", quota_resp.status()));
    }

    let wrapper: QuotaSummaryResponseWrapper = quota_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse QuotaSummary JSON: {}", e))?;

    let raw_groups = wrapper
        .response
        .map(|r| r.groups)
        .or(wrapper.groups)
        .unwrap_or_default();

    let mut model_groups: Vec<ModelQuotaGroup> = Vec::new();
    let mut flat_buckets: Vec<QuotaBucketInfo> = Vec::new();
    let mut overall_5h: Option<i32> = None;
    let mut overall_5h_reset: Option<String> = None;
    let mut overall_weekly: Option<i32> = None;
    let mut overall_weekly_reset: Option<String> = None;

    for g in raw_groups {
        let group_name = g.display_name.unwrap_or_else(|| "General Models".to_string());
        let desc = g.description.unwrap_or_default();

        let mut group_buckets = Vec::new();
        let mut group_5h_pct = None;
        let mut group_5h_reset = None;
        let mut group_5h_desc = None;
        let mut group_wk_pct = None;
        let mut group_wk_reset = None;
        let mut group_wk_desc = None;

        for b in g.buckets {
            let bid = b.bucket_id.unwrap_or_default();
            let bname = b.display_name.unwrap_or_else(|| bid.clone());
            let bdesc = b.description.unwrap_or_default();
            let window = b.window.unwrap_or_default();
            let rem_frac = b.remaining_fraction.unwrap_or(1.0);
            let pct = (rem_frac * 100.0).round().clamp(0.0, 100.0) as i32;
            let reset = b.reset_time.unwrap_or_default();

            let is_5h = window.to_lowercase().contains("5h") || bid.to_lowercase().contains("5h");
            let is_wk = window.to_lowercase().contains("week")
                || window.to_lowercase().contains("7d")
                || bid.to_lowercase().contains("weekly")
                || bid.to_lowercase().contains("7d");

            if is_5h && group_5h_pct.is_none() {
                group_5h_pct = Some(pct);
                if !reset.is_empty() {
                    group_5h_reset = Some(reset.clone());
                }
                if !bdesc.is_empty() {
                    group_5h_desc = Some(bdesc.clone());
                }
            }

            if is_wk && group_wk_pct.is_none() {
                group_wk_pct = Some(pct);
                if !reset.is_empty() {
                    group_wk_reset = Some(reset.clone());
                }
                if !bdesc.is_empty() {
                    group_wk_desc = Some(bdesc.clone());
                }
            }

            flat_buckets.push(QuotaBucketInfo {
                bucket_id: bid.clone(),
                window: window.clone(),
                remaining_fraction: rem_frac,
                percentage: pct,
                reset_time: reset.clone(),
            });

            group_buckets.push(ModelQuotaBucket {
                bucket_id: bid,
                display_name: bname,
                window,
                remaining_fraction: rem_frac,
                percentage: pct,
                reset_time: reset,
                description: bdesc,
            });
        }

        // Match associated models to this group
        let matching_models: Vec<ModelItemInfo> = all_models
            .iter()
            .filter(|m| map_model_to_group(&m.display_name, &m.id, &group_name))
            .cloned()
            .collect();

        // Overall quota priority: prioritize Gemini Models for primary indicator
        if group_name.to_lowercase().contains("gemini") && !group_name.to_lowercase().contains("argon") {
            if group_5h_pct.is_some() {
                overall_5h = group_5h_pct;
                overall_5h_reset = group_5h_reset.clone();
            }
            if group_wk_pct.is_some() {
                overall_weekly = group_wk_pct;
                overall_weekly_reset = group_wk_reset.clone();
            }
        }

        model_groups.push(ModelQuotaGroup {
            group_name,
            description: desc,
            weekly_percentage: group_wk_pct,
            weekly_reset: group_wk_reset,
            weekly_desc: group_wk_desc,
            rolling_5h_percentage: group_5h_pct,
            rolling_5h_reset: group_5h_reset,
            rolling_5h_desc: group_5h_desc,
            buckets: group_buckets,
            models: matching_models,
        });
    }

    if overall_5h.is_none() && !model_groups.is_empty() {
        overall_5h = model_groups[0].rolling_5h_percentage;
        overall_5h_reset = model_groups[0].rolling_5h_reset.clone();
    }
    if overall_weekly.is_none() && !model_groups.is_empty() {
        overall_weekly = model_groups[0].weekly_percentage;
        overall_weekly_reset = model_groups[0].weekly_reset.clone();
    }

    Ok(AccountQuotaSummary {
        email: detected_email,
        name: detected_name,
        tier: detected_tier,
        rolling_5h_percentage: overall_5h,
        rolling_5h_reset_time: overall_5h_reset,
        weekly_percentage: overall_weekly,
        weekly_reset_time: overall_weekly_reset,
        buckets: flat_buckets,
        model_groups,
        prompt_credits,
        flow_credits,
        last_checked: now_secs(),
    })
}

/// Fetches quota summary directly from Google Cloud Code upstream API (when Antigravity is closed or for background accounts)
pub async fn fetch_upstream_quota(
    access_token: &str,
    email: &str,
) -> Result<AccountQuotaSummary, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("HTTP client build failed: {}", e))?;

    let payload = json!({});
    let mut last_error = String::new();

    for endpoint in QUOTA_SUMMARY_ENDPOINTS {
        let resp_res = client
            .post(*endpoint)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", access_token))
            .header(reqwest::header::USER_AGENT, "antigravity/2.29.1")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await;

        let resp = match resp_res {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("Endpoint {} error: {}", endpoint, e);
                continue;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            last_error = format!("Endpoint {} returned HTTP {}", endpoint, status);
            continue;
        }

        let wrapper: QuotaSummaryResponseWrapper = match resp.json().await {
            Ok(s) => s,
            Err(e) => {
                last_error = format!("JSON decode error from {}: {}", endpoint, e);
                continue;
            }
        };

        let raw_groups = wrapper
            .response
            .map(|r| r.groups)
            .or(wrapper.groups)
            .unwrap_or_default();

        let mut model_groups: Vec<ModelQuotaGroup> = Vec::new();
        let mut flat_buckets = Vec::new();
        let mut overall_5h: Option<i32> = None;
        let mut overall_5h_reset: Option<String> = None;
        let mut overall_weekly: Option<i32> = None;
        let mut overall_weekly_reset: Option<String> = None;

        for g in raw_groups {
            let group_name = g.display_name.unwrap_or_else(|| "General Models".to_string());
            let desc = g.description.unwrap_or_default();

            let mut group_buckets = Vec::new();
            let mut group_5h_pct = None;
            let mut group_5h_reset = None;
            let mut group_5h_desc = None;
            let mut group_wk_pct = None;
            let mut group_wk_reset = None;
            let mut group_wk_desc = None;

            for b in g.buckets {
                let bid = b.bucket_id.unwrap_or_default();
                let bname = b.display_name.unwrap_or_else(|| bid.clone());
                let bdesc = b.description.unwrap_or_default();
                let window = b.window.unwrap_or_default();
                let rem_frac = b.remaining_fraction.unwrap_or(1.0);
                let pct = (rem_frac * 100.0).round().clamp(0.0, 100.0) as i32;
                let reset = b.reset_time.unwrap_or_default();

                let is_5h = window.to_lowercase().contains("5h") || bid.to_lowercase().contains("5h");
                let is_weekly = window.to_lowercase().contains("week")
                    || window.to_lowercase().contains("7d")
                    || bid.to_lowercase().contains("weekly")
                    || bid.to_lowercase().contains("7d");

                if is_5h && group_5h_pct.is_none() {
                    group_5h_pct = Some(pct);
                    if !reset.is_empty() {
                        group_5h_reset = Some(reset.clone());
                    }
                    if !bdesc.is_empty() {
                        group_5h_desc = Some(bdesc.clone());
                    }
                }

                if is_weekly && group_wk_pct.is_none() {
                    group_wk_pct = Some(pct);
                    if !reset.is_empty() {
                        group_wk_reset = Some(reset.clone());
                    }
                    if !bdesc.is_empty() {
                        group_wk_desc = Some(bdesc.clone());
                    }
                }

                flat_buckets.push(QuotaBucketInfo {
                    bucket_id: bid.clone(),
                    window: window.clone(),
                    remaining_fraction: rem_frac,
                    percentage: pct,
                    reset_time: reset.clone(),
                });

                group_buckets.push(ModelQuotaBucket {
                    bucket_id: bid,
                    display_name: bname,
                    window,
                    remaining_fraction: rem_frac,
                    percentage: pct,
                    reset_time: reset,
                    description: bdesc,
                });
            }

            // Default model placeholders per group when offline
            let default_models = if group_name.to_lowercase().contains("argon") {
                vec![
                    ModelItemInfo {
                        id: "gemini-4-argon-medium".into(),
                        display_name: "Gemini 4 Argon (Medium)".into(),
                        tag_title: Some("New".into()),
                        percentage: group_wk_pct,
                        reset_time: group_wk_reset.clone(),
                        ..Default::default()
                    },
                    ModelItemInfo {
                        id: "gemini-4-argon-high".into(),
                        display_name: "Gemini 4 Argon (High)".into(),
                        tag_title: Some("New".into()),
                        percentage: group_wk_pct,
                        reset_time: group_wk_reset.clone(),
                        ..Default::default()
                    },
                ]
            } else if group_name.to_lowercase().contains("claude") || group_name.to_lowercase().contains("gpt") {
                vec![
                    ModelItemInfo {
                        id: "claude-opus-5-5-medium".into(),
                        display_name: "Claude Opus 5.5 (Medium)".into(),
                        tag_title: Some("New".into()),
                        percentage: group_wk_pct,
                        reset_time: group_wk_reset.clone(),
                        ..Default::default()
                    },
                    ModelItemInfo {
                        id: "claude-sonnet-5-5-medium".into(),
                        display_name: "Claude Sonnet 5.5 (Medium)".into(),
                        tag_title: Some("New".into()),
                        percentage: group_wk_pct,
                        reset_time: group_wk_reset.clone(),
                        ..Default::default()
                    },
                    ModelItemInfo {
                        id: "gpt-oss-120b-medium".into(),
                        display_name: "GPT-OSS 120B (Medium)".into(),
                        tag_title: Some("Notice".into()),
                        percentage: group_wk_pct,
                        reset_time: group_wk_reset.clone(),
                        ..Default::default()
                    },
                ]
            } else {
                vec![
                    ModelItemInfo {
                        id: "gemini-3.8-flash-high".into(),
                        display_name: "Gemini 3.8 Flash (High)".into(),
                        percentage: group_5h_pct,
                        reset_time: group_5h_reset.clone(),
                        ..Default::default()
                    },
                    ModelItemInfo {
                        id: "gemini-3.8-flash-medium".into(),
                        display_name: "Gemini 3.8 Flash (Medium)".into(),
                        percentage: group_5h_pct,
                        reset_time: group_5h_reset.clone(),
                        ..Default::default()
                    },
                    ModelItemInfo {
                        id: "gemini-2.5-pro".into(),
                        display_name: "Gemini 2.5 Pro".into(),
                        percentage: group_5h_pct,
                        reset_time: group_5h_reset.clone(),
                        ..Default::default()
                    },
                ]
            };

            if group_name.to_lowercase().contains("gemini") && !group_name.to_lowercase().contains("argon") {
                if group_5h_pct.is_some() {
                    overall_5h = group_5h_pct;
                    overall_5h_reset = group_5h_reset.clone();
                }
                if group_wk_pct.is_some() {
                    overall_weekly = group_wk_pct;
                    overall_weekly_reset = group_wk_reset.clone();
                }
            }

            model_groups.push(ModelQuotaGroup {
                group_name,
                description: desc,
                weekly_percentage: group_wk_pct,
                weekly_reset: group_wk_reset,
                weekly_desc: group_wk_desc,
                rolling_5h_percentage: group_5h_pct,
                rolling_5h_reset: group_5h_reset,
                rolling_5h_desc: group_5h_desc,
                buckets: group_buckets,
                models: default_models,
            });
        }

        if overall_5h.is_none() && !model_groups.is_empty() {
            overall_5h = model_groups[0].rolling_5h_percentage;
            overall_5h_reset = model_groups[0].rolling_5h_reset.clone();
        }
        if overall_weekly.is_none() && !model_groups.is_empty() {
            overall_weekly = model_groups[0].weekly_percentage;
            overall_weekly_reset = model_groups[0].weekly_reset.clone();
        }

        return Ok(AccountQuotaSummary {
            email: email.to_string(),
            name: None,
            tier: None,
            rolling_5h_percentage: overall_5h,
            rolling_5h_reset_time: overall_5h_reset,
            weekly_percentage: overall_weekly,
            weekly_reset_time: overall_weekly_reset,
            buckets: flat_buckets,
            model_groups,
            prompt_credits: None,
            flow_credits: None,
            last_checked: now_secs(),
        });
    }

    Err(format!("All quota endpoints failed. Last error: {}", last_error))
}
