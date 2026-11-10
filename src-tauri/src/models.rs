use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaBucketInfo {
    pub bucket_id: String,
    pub window: String,             // "5h", "weekly", "7d", etc.
    pub remaining_fraction: f64,   // 0.0 to 1.0
    pub percentage: i32,           // 0 to 100
    pub reset_time: String,        // RFC3339
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelItemInfo {
    pub id: String,                 // e.g. "gemini-4-argon-medium"
    pub display_name: String,       // e.g. "Gemini 4 Argon (Medium)"
    pub remaining_fraction: Option<f64>,
    pub percentage: Option<i32>,
    pub reset_time: Option<String>,
    pub tag_title: Option<String>,  // e.g. "New", "Notice"
    pub tag_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQuotaBucket {
    pub bucket_id: String,
    pub display_name: String,
    pub window: String,             // "5h", "weekly"
    pub remaining_fraction: f64,
    pub percentage: i32,
    pub reset_time: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelQuotaGroup {
    pub group_name: String,         // "Gemini Models", "Claude and GPT models", "Gemini 4 Argon"
    pub description: String,
    pub weekly_percentage: Option<i32>,
    pub weekly_reset: Option<String>,
    pub weekly_desc: Option<String>,
    pub rolling_5h_percentage: Option<i32>,
    pub rolling_5h_reset: Option<String>,
    pub rolling_5h_desc: Option<String>,
    pub buckets: Vec<ModelQuotaBucket>,
    pub models: Vec<ModelItemInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQuotaSummary {
    pub email: String,
    pub name: Option<String>,
    pub tier: Option<String>,
    pub rolling_5h_percentage: Option<i32>,
    pub rolling_5h_reset_time: Option<String>,
    pub weekly_percentage: Option<i32>,
    pub weekly_reset_time: Option<String>,
    pub buckets: Vec<QuotaBucketInfo>,
    pub model_groups: Vec<ModelQuotaGroup>,
    pub prompt_credits: Option<i64>,
    pub flow_credits: Option<i64>,
    pub last_checked: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountProfile {
    pub id: String,
    pub label: String,
    pub email: String,
    pub name: String,
    pub tier: String,
    pub picture: Option<String>,
    
    // Antigravity credentials
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expiry_timestamp: Option<i64>,
    pub id_token: Option<String>,

    // Legacy / DB payload compatibility
    pub oauth_token: Option<String>,
    pub user_status: Option<String>,
    pub model_credits: Option<String>,
    pub model_preferences: Option<String>,

    // State & Rate limiting
    pub is_active: bool,
    pub is_exhausted: bool,
    pub exhausted_until: Option<i64>, // Unix timestamp in seconds
    pub last_used: i64,
    pub created_at: i64,

    // Upstream & Local Model Quotas
    pub rolling_5h_percentage: Option<i32>,
    pub rolling_5h_reset: Option<String>,
    pub weekly_percentage: Option<i32>,
    pub weekly_reset: Option<String>,
    pub quota_buckets: Vec<QuotaBucketInfo>,
    #[serde(default)]
    pub model_groups: Vec<ModelQuotaGroup>,
    pub prompt_credits: Option<i64>,
    pub flow_credits: Option<i64>,
    pub quota_last_updated: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSession {
    pub email: String,
    pub name: String,
    pub tier: String,
    pub picture: Option<String>,
    pub has_access_token: bool,
    pub has_refresh_token: bool,
    pub app_storage_path: Option<String>,
    pub jetski_token_path: Option<String>,
    pub db_path: Option<String>,
    pub is_saved: bool,
    pub matched_profile_id: Option<String>,
    pub rolling_5h_percentage: Option<i32>,
    pub rolling_5h_reset: Option<String>,
    pub weekly_percentage: Option<i32>,
    pub weekly_reset: Option<String>,
    #[serde(default)]
    pub model_groups: Vec<ModelQuotaGroup>,
    pub prompt_credits: Option<i64>,
    pub flow_credits: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_switch_on_limit: bool,
    pub notify_on_switch: bool,
    pub monitor_interval_secs: u64,
    pub default_cooldown_mins: u32,
    pub enable_upstream_quota_poller: bool,
    pub quota_refresh_interval_mins: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_switch_on_limit: true,
            notify_on_switch: true,
            monitor_interval_secs: 5,
            default_cooldown_mins: 300, // 5 hours default reset window
            enable_upstream_quota_poller: true,
            quota_refresh_interval_mins: 15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchResult {
    pub success: bool,
    pub message: String,
    pub profile: Option<AccountProfile>,
    pub hot_switched: bool,
}
