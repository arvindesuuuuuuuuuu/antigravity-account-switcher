import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface ModelItemInfo {
  id: string;
  display_name: string;
  remaining_fraction?: number;
  percentage?: number;
  reset_time?: string;
  tag_title?: string;
  tag_description?: string;
}

interface ModelQuotaBucket {
  bucket_id: string;
  display_name: string;
  window: string;
  remaining_fraction: number;
  percentage: number;
  reset_time: string;
  description: string;
}

interface ModelQuotaGroup {
  group_name: string;
  description: string;
  weekly_percentage?: number;
  weekly_reset?: string;
  weekly_desc?: string;
  rolling_5h_percentage?: number;
  rolling_5h_reset?: string;
  rolling_5h_desc?: string;
  buckets: ModelQuotaBucket[];
  models: ModelItemInfo[];
}

interface QuotaBucketInfo {
  bucket_id: string;
  window: string;
  remaining_fraction: number;
  percentage: number;
  reset_time: string;
}

interface AccountProfile {
  id: string;
  label: string;
  email: string;
  name: string;
  tier: string;
  picture?: string;
  access_token?: string;
  refresh_token?: string;
  is_active: boolean;
  is_exhausted: boolean;
  exhausted_until?: number;
  last_used: number;
  created_at: number;
  rolling_5h_percentage?: number;
  rolling_5h_reset?: string;
  weekly_percentage?: number;
  weekly_reset?: string;
  quota_buckets: QuotaBucketInfo[];
  model_groups: ModelQuotaGroup[];
  prompt_credits?: number;
  flow_credits?: number;
}

interface ActiveSession {
  email: string;
  name: string;
  tier: string;
  picture?: string;
  has_access_token: boolean;
  has_refresh_token: boolean;
  app_storage_path?: string;
  jetski_token_path?: string;
  db_path?: string;
  is_saved: boolean;
  matched_profile_id?: string;
  rolling_5h_percentage?: number;
  rolling_5h_reset?: string;
  weekly_percentage?: number;
  weekly_reset?: string;
  model_groups: ModelQuotaGroup[];
  prompt_credits?: number;
  flow_credits?: number;
}

interface AppSettings {
  auto_switch_on_limit: boolean;
  notify_on_switch: boolean;
  monitor_interval_secs: number;
  default_cooldown_mins: number;
}

interface SwitchResult {
  success: boolean;
  message: string;
  profile?: AccountProfile;
  hot_switched: boolean;
}

// App State
let activeSession: ActiveSession | null = null;
let profiles: AccountProfile[] = [];
let appSettings: AppSettings = {
  auto_switch_on_limit: true,
  notify_on_switch: true,
  monitor_interval_secs: 5,
  default_cooldown_mins: 300,
};
let isRefreshingQuotas = false;
let activeModalTab: "capture" | "token" = "capture";

// DOM Elements
const activeNameEl = document.getElementById("activeName")!;
const activeEmailEl = document.getElementById("activeEmail")!;
const activeTierEl = document.getElementById("activeTier")!;
const activeAvatarEl = document.getElementById("activeAvatar") as HTMLImageElement;
const activeAvatarFallbackEl = document.getElementById("activeAvatarFallback")!;
const savedStatusBadgeEl = document.getElementById("savedStatusBadge")!;
const rateLimitBadgeEl = document.getElementById("rateLimitBadge")!;
const promptCreditsBadgeEl = document.getElementById("promptCreditsBadge")!;
const flowCreditsBadgeEl = document.getElementById("flowCreditsBadge")!;
const activeModelGroupsListEl = document.getElementById("activeModelGroupsList")!;
const liveRpcIndicatorEl = document.getElementById("liveRpcIndicator")!;
const profileCountEl = document.getElementById("profileCount")!;
const profilesListEl = document.getElementById("profilesList")!;
const autoSwitchToggleEl = document.getElementById("autoSwitchToggle") as HTMLInputElement;

// Modal Elements
const addModal = document.getElementById("addModal")!;
const profileLabelInput = document.getElementById("profileLabelInput") as HTMLInputElement;
const tabCaptureBtn = document.getElementById("tabCaptureBtn")!;
const tabTokenBtn = document.getElementById("tabTokenBtn")!;
const tabCaptureContent = document.getElementById("tabCaptureContent")!;
const tabTokenContent = document.getElementById("tabTokenContent")!;
const tokenLabelInput = document.getElementById("tokenLabelInput") as HTMLInputElement;
const tokenEmailInput = document.getElementById("tokenEmailInput") as HTMLInputElement;
const tokenAccessInput = document.getElementById("tokenAccessInput") as HTMLInputElement;
const tokenRefreshInput = document.getElementById("tokenRefreshInput") as HTMLInputElement;

const settingsModal = document.getElementById("settingsModal")!;
const cooldownSelect = document.getElementById("cooldownSelect") as HTMLSelectElement;
const notifySwitchCheckbox = document.getElementById("notifySwitchCheckbox") as HTMLInputElement;
const diagStoragePath = document.getElementById("diagStoragePath")!;
const toastContainer = document.getElementById("toastContainer")!;

function showToast(message: string, type: "success" | "alert" | "info" = "info") {
  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.innerHTML = `<span>${type === "alert" ? "⚠️" : type === "success" ? "✅" : "ℹ️"}</span> <span>${message}</span>`;
  toastContainer.appendChild(toast);
  setTimeout(() => {
    toast.style.opacity = "0";
    toast.style.transform = "translateY(10px)";
    setTimeout(() => toast.remove(), 300);
  }, 4000);
}

function formatCountdown(untilTimestampSecs: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = untilTimestampSecs - now;
  if (diff <= 0) return "Resetting...";
  const hours = Math.floor(diff / 3600);
  const minutes = Math.floor((diff % 3600) / 60);
  const seconds = diff % 60;
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m ${seconds}s`;
}

function formatRfcReset(rfcTime?: string): string {
  if (!rfcTime) return "Ready";
  try {
    const target = new Date(rfcTime).getTime();
    const diff = Math.floor((target - Date.now()) / 1000);
    if (diff <= 0) return "Ready";
    const hours = Math.floor(diff / 3600);
    const mins = Math.floor((diff % 3600) / 60);
    if (hours > 24) {
      const days = Math.floor(hours / 24);
      return `Resets in ${days}d ${hours % 24}h`;
    }
    if (hours > 0) return `Resets in ${hours}h ${mins}m`;
    return `Resets in ${mins}m`;
  } catch {
    return "Ready";
  }
}

function getPctClass(pct: number): "high" | "mid" | "low" {
  if (pct >= 50) return "high";
  if (pct >= 20) return "mid";
  return "low";
}

async function loadActiveSession() {
  try {
    const session = await invoke<ActiveSession>("cmd_get_active_session");
    activeSession = session;
    renderActiveSession();
  } catch (err) {
    console.error("Failed to load active session:", err);
    activeNameEl.textContent = "Antigravity Not Detected";
    activeEmailEl.textContent = String(err);
  }
}

async function loadProfiles() {
  try {
    profiles = await invoke<AccountProfile[]>("cmd_list_profiles");
    renderProfiles();
  } catch (err) {
    console.error("Failed to load profiles:", err);
  }
}

async function loadSettings() {
  try {
    appSettings = await invoke<AppSettings>("cmd_get_settings");
    autoSwitchToggleEl.checked = appSettings.auto_switch_on_limit;
    cooldownSelect.value = String(appSettings.default_cooldown_mins);
    notifySwitchCheckbox.checked = appSettings.notify_on_switch;
  } catch (err) {
    console.error("Failed to load settings:", err);
  }
}

function renderActiveSession() {
  if (!activeSession) return;

  activeNameEl.textContent = activeSession.name || "Antigravity User";
  activeEmailEl.textContent = activeSession.email;
  activeTierEl.textContent = activeSession.tier || "Standard";

  if (activeSession.picture) {
    activeAvatarEl.src = activeSession.picture;
    activeAvatarEl.style.display = "block";
    activeAvatarFallbackEl.style.display = "none";
  } else {
    activeAvatarFallbackEl.textContent = (activeSession.name || activeSession.email || "A")[0].toUpperCase();
    activeAvatarFallbackEl.style.display = "flex";
    activeAvatarEl.style.display = "none";
  }

  diagStoragePath.textContent = activeSession.app_storage_path || "~/.config/Antigravity";

  if (activeSession.is_saved) {
    savedStatusBadgeEl.textContent = "Saved Profile";
    savedStatusBadgeEl.className = "meta-tag";
  } else {
    savedStatusBadgeEl.textContent = "Unsaved Session";
    savedStatusBadgeEl.className = "meta-tag health-exhausted";
  }

  // Display Credits Tags if available
  if (activeSession.prompt_credits != null) {
    promptCreditsBadgeEl.textContent = `${activeSession.prompt_credits.toLocaleString()} Prompts`;
    promptCreditsBadgeEl.style.display = "inline-flex";
  } else {
    promptCreditsBadgeEl.style.display = "none";
  }

  if (activeSession.flow_credits != null) {
    flowCreditsBadgeEl.textContent = `${activeSession.flow_credits.toLocaleString()} Flow`;
    flowCreditsBadgeEl.style.display = "inline-flex";
  } else {
    flowCreditsBadgeEl.style.display = "none";
  }

  // Determine rate limit status across groups
  let isExhausted = false;
  if (activeSession.model_groups && activeSession.model_groups.length > 0) {
    for (const g of activeSession.model_groups) {
      if ((g.rolling_5h_percentage != null && g.rolling_5h_percentage <= 0) ||
          (g.weekly_percentage != null && g.weekly_percentage <= 0)) {
        isExhausted = true;
        break;
      }
    }
  }

  if (isExhausted) {
    rateLimitBadgeEl.textContent = "Usage Limit Reached";
    rateLimitBadgeEl.className = "meta-tag health-exhausted";
  } else {
    rateLimitBadgeEl.textContent = "Quota Available";
    rateLimitBadgeEl.className = "meta-tag health-ok";
  }

  // Render Live Model Groups Breakdown
  renderModelGroupsList(activeModelGroupsListEl, activeSession.model_groups);
}

function renderModelGroupsList(container: HTMLElement, groups?: ModelQuotaGroup[]) {
  container.innerHTML = "";

  if (!groups || groups.length === 0) {
    container.innerHTML = `
      <div class="empty-state" style="padding: 12px; margin: 0;">
        <p style="font-size: 0.75rem; color: var(--text-muted);">
          Antigravity model quotas will appear once local Language Server is connected.
        </p>
      </div>
    `;
    return;
  }

  groups.forEach((group) => {
    const card = document.createElement("div");
    card.className = "model-group-card";

    // Build quota bars HTML
    let barsHtml = "";

    if (group.weekly_percentage != null) {
      const p = group.weekly_percentage;
      const resetText = formatRfcReset(group.weekly_reset);
      barsHtml += `
        <div class="group-quota-row">
          <div class="group-quota-labels">
            <span class="group-quota-title">Weekly limit</span>
            <span class="group-quota-val ${getPctClass(p)}">${p}% left</span>
          </div>
          <div class="quota-progress-track">
            <div class="quota-progress-fill ${getPctClass(p)}" style="width: ${p}%;"></div>
          </div>
          <div class="group-quota-reset">${resetText}</div>
        </div>
      `;
    }

    if (group.rolling_5h_percentage != null) {
      const p = group.rolling_5h_percentage;
      const resetText = formatRfcReset(group.rolling_5h_reset);
      barsHtml += `
        <div class="group-quota-row">
          <div class="group-quota-labels">
            <span class="group-quota-title">5h limit</span>
            <span class="group-quota-val ${getPctClass(p)}">${p}% left</span>
          </div>
          <div class="quota-progress-track">
            <div class="quota-progress-fill ${getPctClass(p)}" style="width: ${p}%;"></div>
          </div>
          <div class="group-quota-reset">${resetText}</div>
        </div>
      `;
    }

    // Build model chips HTML
    let chipsHtml = "";
    if (group.models && group.models.length > 0) {
      chipsHtml = `<div class="model-chips-row">` + group.models.map((m) => {
        let tagHtml = "";
        if (m.tag_title) {
          const tagClass = m.tag_title.toLowerCase().includes("new") ? "tag-new" : "tag-notice";
          tagHtml = `<span class="model-tag ${tagClass}">${escapeHtml(m.tag_title)}</span>`;
        }
        return `
          <div class="model-chip" title="${escapeHtml(m.display_name)}">
            <span>${escapeHtml(m.display_name)}</span>
            ${tagHtml}
          </div>
        `;
      }).join("") + `</div>`;
    }

    card.innerHTML = `
      <div class="model-group-head">
        <span class="model-group-name">${escapeHtml(group.group_name)}</span>
        <span class="model-group-desc">${escapeHtml(group.description || "")}</span>
      </div>
      <div class="group-quota-bars">
        ${barsHtml}
      </div>
      ${chipsHtml}
    `;

    container.appendChild(card);
  });
}

function renderProfiles() {
  profileCountEl.textContent = String(profiles.length);
  profilesListEl.innerHTML = "";

  if (profiles.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.innerHTML = `
      <div class="empty-icon">👥</div>
      <h3>No profiles saved yet</h3>
      <p>Click "Save" above to capture your current signed-in Google account!</p>
    `;
    profilesListEl.appendChild(empty);
    return;
  }

  profiles.forEach((profile) => {
    const card = document.createElement("div");
    card.className = `profile-card ${profile.is_active ? "active" : ""} ${profile.is_exhausted ? "exhausted" : ""}`;

    let countdownHtml = "";
    if (profile.is_exhausted && profile.exhausted_until) {
      countdownHtml = `<span class="cooldown-badge" data-until="${profile.exhausted_until}">⏳ ${formatCountdown(profile.exhausted_until)}</span>`;
    }

    // Model group quick badges
    let groupPillsHtml = "";
    if (profile.model_groups && profile.model_groups.length > 0) {
      groupPillsHtml = `<div class="model-chips-row" style="margin-top: 6px;">` + profile.model_groups.map((g) => {
        const val = g.weekly_percentage != null ? `${g.weekly_percentage}%` : `${g.rolling_5h_percentage ?? 100}%`;
        const pctVal = g.weekly_percentage ?? g.rolling_5h_percentage ?? 100;
        const shortName = g.group_name.replace(" Models", "").replace(" models", "");
        return `<span class="model-chip"><strong style="color:#fff;">${shortName}:</strong> <span class="group-quota-val ${getPctClass(pctVal)}">${val}</span></span>`;
      }).join("") + `</div>`;
    } else {
      const p5h = profile.rolling_5h_percentage ?? 100;
      const pWk = profile.weekly_percentage ?? 100;
      groupPillsHtml = `
        <div class="model-chips-row" style="margin-top: 6px;">
          <span class="model-chip"><strong>5h Limit:</strong> <span class="${getPctClass(p5h)}">${p5h}%</span></span>
          <span class="model-chip"><strong>Weekly:</strong> <span class="${getPctClass(pWk)}">${pWk}%</span></span>
        </div>
      `;
    }

    card.innerHTML = `
      <div class="profile-card-top">
        <div class="profile-card-info">
          <div class="profile-title-row">
            <span class="profile-label">${escapeHtml(profile.label)}</span>
            ${profile.is_active ? '<span class="status-pill active">ACTIVE</span>' : ""}
            ${profile.is_exhausted ? '<span class="status-pill exhausted">RATE-LIMITED</span>' : ""}
            ${countdownHtml}
          </div>
          <div class="profile-email-row">
            <span class="profile-email">${escapeHtml(profile.email)}</span>
            <span class="profile-tier">${escapeHtml(profile.tier)}</span>
          </div>
        </div>
        <div class="profile-card-actions">
          ${
            !profile.is_active
              ? `<button class="btn btn-primary btn-sm switch-btn" data-id="${profile.id}">⚡ Switch</button>`
              : '<button class="btn btn-secondary btn-sm" disabled>Current</button>'
          }
          <div class="profile-dropdown-container">
            <button class="icon-btn-sm menu-btn" data-id="${profile.id}">⋮</button>
            <div class="profile-menu" id="menu-${profile.id}" style="display:none;">
              ${
                profile.is_exhausted
                  ? `<button class="menu-item mark-available-btn" data-id="${profile.id}">✅ Mark Available</button>`
                  : `<button class="menu-item mark-exhausted-btn" data-id="${profile.id}">⚠️ Mark Rate-Limited</button>`
              }
              <button class="menu-item delete-btn text-danger" data-id="${profile.id}">🗑️ Delete Profile</button>
            </div>
          </div>
        </div>
      </div>

      <!-- Quick Model Groups Quota Display -->
      ${groupPillsHtml}
    `;

    profilesListEl.appendChild(card);
  });

  attachProfileEvents();
}

function escapeHtml(str: string): string {
  return str.replace(/[&<>"']/g, (m) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[m] || m));
}

function attachProfileEvents() {
  document.querySelectorAll(".switch-btn").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      const id = (e.currentTarget as HTMLElement).dataset.id!;
      await handleSwitchAccount(id);
    });
  });

  document.querySelectorAll(".menu-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const id = (e.currentTarget as HTMLElement).dataset.id!;
      const menu = document.getElementById(`menu-${id}`);
      if (menu) {
        document.querySelectorAll(".profile-menu").forEach((m) => {
          if (m !== menu) (m as HTMLElement).style.display = "none";
        });
        menu.style.display = menu.style.display === "none" ? "flex" : "none";
      }
    });
  });

  document.querySelectorAll(".mark-exhausted-btn").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      const id = (e.currentTarget as HTMLElement).dataset.id!;
      try {
        await invoke("cmd_mark_exhausted", { profileId: id, cooldownMinutes: appSettings.default_cooldown_mins });
        showToast("Marked account as rate-limited", "alert");
        await loadProfiles();
      } catch (err) {
        showToast(String(err), "alert");
      }
    });
  });

  document.querySelectorAll(".mark-available-btn").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      const id = (e.currentTarget as HTMLElement).dataset.id!;
      try {
        await invoke("cmd_mark_available", { profileId: id });
        showToast("Marked account as available", "success");
        await loadProfiles();
      } catch (err) {
        showToast(String(err), "alert");
      }
    });
  });

  document.querySelectorAll(".delete-btn").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      const id = (e.currentTarget as HTMLElement).dataset.id!;
      if (confirm("Are you sure you want to delete this profile?")) {
        try {
          await invoke("cmd_delete_profile", { profileId: id });
          showToast("Profile removed", "info");
          await loadProfiles();
          await loadActiveSession();
        } catch (err) {
          showToast(String(err), "alert");
        }
      }
    });
  });
}

// Global click closes all menus
document.addEventListener("click", () => {
  document.querySelectorAll(".profile-menu").forEach((m) => {
    (m as HTMLElement).style.display = "none";
  });
});

async function handleSwitchAccount(profileId: string) {
  try {
    showToast("Switching Antigravity credentials...", "info");
    const result = await invoke<SwitchResult>("cmd_switch_account", { profileId });
    if (result.success) {
      showToast(result.message, "success");
      await loadActiveSession();
      await loadProfiles();
    } else {
      showToast(result.message, "alert");
    }
  } catch (err) {
    showToast(String(err), "alert");
  }
}

async function handleQuickSwitchNext() {
  try {
    showToast("Finding next available Google account...", "info");
    const result = await invoke<SwitchResult>("cmd_auto_switch_next");
    if (result.success) {
      showToast(result.message, "success");
      await loadActiveSession();
      await loadProfiles();
    } else {
      showToast(result.message, "alert");
    }
  } catch (err) {
    showToast(String(err), "alert");
  }
}

// Refresh Live Quota Button
document.getElementById("refreshQuotaBtn")!.addEventListener("click", async () => {
  if (isRefreshingQuotas) return;
  isRefreshingQuotas = true;
  const btn = document.getElementById("refreshQuotaBtn")!;
  btn.classList.add("spinning");

  try {
    showToast("Querying live model quotas from Antigravity...", "info");
    await invoke("cmd_refresh_quotas");
    await loadActiveSession();
    await loadProfiles();
    showToast("Live model quotas updated!", "success");
  } catch (err) {
    showToast(String(err), "alert");
  } finally {
    isRefreshingQuotas = false;
    btn.classList.remove("spinning");
  }
});

// Save Current Account Button
document.getElementById("saveCurrentBtn")!.addEventListener("click", async () => {
  try {
    const defaultLabel = activeSession?.name && activeSession.name !== "Antigravity User"
      ? `${activeSession.name} (${activeSession.tier})`
      : "";
    profileLabelInput.value = defaultLabel;
    activeModalTab = "capture";
    switchModalTab("capture");
    addModal.style.display = "flex";
  } catch (err) {
    showToast(String(err), "alert");
  }
});

// Quick Switch Next Button
document.getElementById("quickSwitchNextBtn")!.addEventListener("click", handleQuickSwitchNext);

// Add Account Button
document.getElementById("addAccountBtn")!.addEventListener("click", () => {
  profileLabelInput.value = "";
  tokenLabelInput.value = "";
  tokenEmailInput.value = "";
  tokenAccessInput.value = "";
  tokenRefreshInput.value = "";
  activeModalTab = "capture";
  switchModalTab("capture");
  addModal.style.display = "flex";
});

// Modal Actions
function switchModalTab(tab: "capture" | "token") {
  activeModalTab = tab;
  if (tab === "capture") {
    tabCaptureBtn.classList.add("active");
    tabTokenBtn.classList.remove("active");
    tabCaptureContent.style.display = "block";
    tabTokenContent.style.display = "none";
  } else {
    tabTokenBtn.classList.add("active");
    tabCaptureBtn.classList.remove("active");
    tabCaptureContent.style.display = "none";
    tabTokenContent.style.display = "block";
  }
}

tabCaptureBtn.addEventListener("click", () => switchModalTab("capture"));
tabTokenBtn.addEventListener("click", () => switchModalTab("token"));

document.getElementById("closeModalBtn")!.addEventListener("click", () => {
  addModal.style.display = "none";
});

document.getElementById("cancelModalBtn")!.addEventListener("click", () => {
  addModal.style.display = "none";
});

document.getElementById("confirmSaveProfileBtn")!.addEventListener("click", async () => {
  if (activeModalTab === "capture") {
    const label = profileLabelInput.value.trim() || undefined;
    try {
      const saved = await invoke<AccountProfile>("cmd_save_current_account", { label });
      showToast(`Account '${saved.label}' saved successfully!`, "success");
      addModal.style.display = "none";
      await loadActiveSession();
      await loadProfiles();
    } catch (err) {
      showToast(String(err), "alert");
    }
  } else {
    const label = tokenLabelInput.value.trim();
    const email = tokenEmailInput.value.trim();
    const access = tokenAccessInput.value.trim();
    const refresh = tokenRefreshInput.value.trim();

    if (!label || !email || !access) {
      showToast("Label, Email, and Access Token are required", "alert");
      return;
    }

    try {
      const saved = await invoke<AccountProfile>("cmd_add_account_tokens", {
        label,
        email,
        accessToken: access,
        refreshToken: refresh,
        idToken: undefined,
      });
      showToast(`Account '${saved.label}' added!`, "success");
      addModal.style.display = "none";
      await loadProfiles();
    } catch (err) {
      showToast(String(err), "alert");
    }
  }
});

// Settings Modal Actions
document.getElementById("settingsBtn")!.addEventListener("click", () => {
  settingsModal.style.display = "flex";
});

document.getElementById("closeSettingsBtn")!.addEventListener("click", () => {
  settingsModal.style.display = "none";
});

document.getElementById("saveSettingsBtn")!.addEventListener("click", async () => {
  try {
    appSettings.default_cooldown_mins = parseInt(cooldownSelect.value, 10);
    appSettings.notify_on_switch = notifySwitchCheckbox.checked;
    await invoke("cmd_save_settings", { settings: appSettings });
    showToast("Settings saved", "success");
    settingsModal.style.display = "none";
  } catch (err) {
    showToast(String(err), "alert");
  }
});

// Auto-switch toggle
autoSwitchToggleEl.addEventListener("change", async () => {
  appSettings.auto_switch_on_limit = autoSwitchToggleEl.checked;
  try {
    await invoke("cmd_save_settings", { settings: appSettings });
    showToast(
      appSettings.auto_switch_on_limit ? "Auto-switch enabled on quota limits" : "Auto-switch disabled",
      "info"
    );
  } catch (err) {
    console.error("Failed to save auto-switch toggle:", err);
  }
});

// Periodic Countdown Updater for Rate-Limited Badges
setInterval(() => {
  document.querySelectorAll(".cooldown-badge").forEach((el) => {
    const until = parseInt((el as HTMLElement).dataset.until || "0", 10);
    if (until > 0) {
      (el as HTMLElement).textContent = `⏳ ${formatCountdown(until)}`;
    }
  });
}, 1000);

// Tauri Background Event Listeners
async function setupEventListeners() {
  try {
    await listen("rate-limit-detected", async (event: any) => {
      console.log("[Event] Rate limit detected:", event.payload);
      showToast("Rate limit detected in Antigravity! Checking alternatives...", "alert");
      await loadActiveSession();
      await loadProfiles();
    });

    await listen("account-switched", async (event: any) => {
      console.log("[Event] Account switched:", event.payload);
      showToast(event.payload.message || "Account switched successfully", "success");
      await loadActiveSession();
      await loadProfiles();
    });

    await listen("auto-switch-failed", (event: any) => {
      showToast(event.payload.error || "Auto-switch failed", "alert");
    });
  } catch (e) {
    console.warn("Event listener registration deferred:", e);
  }
}

// App Initialization
async function init() {
  await loadSettings();
  await loadActiveSession();
  await loadProfiles();
  await setupEventListeners();

  // Auto-refresh quotas on launch if connected
  setTimeout(async () => {
    try {
      await invoke("cmd_refresh_quotas");
      await loadActiveSession();
      await loadProfiles();
    } catch {
      // Quiet background refresh
    }
  }, 1000);
}

window.addEventListener("DOMContentLoaded", init);
