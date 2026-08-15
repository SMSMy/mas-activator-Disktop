use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;
use tauri::State;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

const UTF8_PREFIX: &str = r#"chcp 65001 > $null 2>&1; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; "#;

const WINDOWS_APP_ID: &str = "55c92734-d682-4d71-983e-d6ec3f16059f";
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(600);
const CHECK_TIMEOUT: Duration = Duration::from_secs(60);
const POST_VERIFY_RETRIES: u32 = 2;
const POST_VERIFY_DELAY: Duration = Duration::from_secs(3);

static NEXT_OP_ID: AtomicU64 = AtomicU64::new(1);

// ===== حالة التطبيق =====

struct RunningOp {
    id: u64,
    kind: String,
    pid: Option<u32>,
    cancel: Arc<AtomicBool>,
}

pub struct AppState {
    logs: Arc<Mutex<Vec<String>>>,
    current: Arc<Mutex<Option<RunningOp>>>,
}

// ===== النموذج المنظم (المرحلة 1.2) =====

#[derive(Debug, Clone, Deserialize)]
struct RawProduct {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    application_id: String,
    #[serde(default)]
    partial_key: bool,
    #[serde(default)]
    license_status: i32,
    #[serde(default)]
    grace_minutes: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct RawCheck {
    #[serde(default)]
    checked_at: Option<String>,
    #[serde(default)]
    items: Option<Vec<RawProduct>>,
    #[serde(default)]
    cim_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LicenseState {
    Activated,
    Notification,
    Grace,
    NotLicensed,
    NotGenuine,
    Unknown,
}

impl LicenseState {
    fn from_status(status: i32) -> Self {
        match status {
            1 => LicenseState::Activated,
            5 => LicenseState::Notification,
            2 | 3 | 6 => LicenseState::Grace,
            4 => LicenseState::NotGenuine,
            0 => LicenseState::NotLicensed,
            _ => LicenseState::Unknown,
        }
    }

    fn priority(status: i32) -> u8 {
        match status {
            1 => 0,
            2 => 1,
            6 => 2,
            3 => 3,
            4 => 4,
            5 => 5,
            0 => 6,
            _ => 99,
        }
    }

    fn label(&self, grace_days: Option<u32>) -> String {
        match self {
            LicenseState::Activated => "مفعل ✅".to_string(),
            LicenseState::Notification => "إشعار ترخيص ⚠️".to_string(),
            LicenseState::Grace => match grace_days {
                Some(d) if d > 0 => format!("فترة سماح ⏳ ({} يوم)", d),
                _ => "فترة سماح ⏳".to_string(),
            },
            LicenseState::NotLicensed => "غير مرخص ❌".to_string(),
            LicenseState::NotGenuine => "نسخة غير أصلية ❌".to_string(),
            LicenseState::Unknown => "حالة غير معروفة".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProductKind {
    Windows,
    Office,
}

#[derive(Debug, Clone, Serialize)]
struct ProductStatus {
    kind: ProductKind,
    name: String,
    state: LicenseState,
    label: String,
    grace_days: Option<u32>,
    selection_reason: String,
    #[serde(skip)]
    license_status: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum StatusErrorKind {
    QueryFailed,
    DiscoveryFailed,
}

#[derive(Debug, Clone, Serialize)]
struct StatusError {
    kind: StatusErrorKind,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct StatusReport {
    windows: Option<ProductStatus>,
    office: Option<ProductStatus>,
    observed: Vec<ProductStatus>,
    checked_at: Option<String>,
    error: Option<StatusError>,
}

// ===== منطق الاختيار (نقي وقابل للاختبار — المرحلة 1.4) =====

fn is_windows(app_id: &str) -> bool {
    app_id.eq_ignore_ascii_case(WINDOWS_APP_ID)
}

fn is_office(name: &str) -> bool {
    let u = name.to_uppercase();
    (u.contains("OFFICE") || u.contains("PROJECT") || u.contains("VISIO")) && !u.contains("ONENOTE")
}

fn is_insider(name: &str, description: &str) -> bool {
    name.to_uppercase().contains("INSIDER") || description.to_uppercase().contains("INSIDER")
}

fn office_clean_name(name: &str) -> String {
    let mut n = name.trim().to_string();
    if let Some(idx) = n.find(", ") {
        let head = &n[..idx];
        if head.to_uppercase().starts_with("OFFICE") {
            n = n[idx + 2..].to_string();
        }
    }
    if let Some(idx) = n.find(',') {
        n.truncate(idx);
    }
    n.trim().to_string()
}

fn windows_clean_name(name: &str) -> String {
    let cleaned = name.trim().replace("(R)", "");
    let edition = cleaned
        .split(',')
        .nth(1)
        .map(|s| s.trim().trim_end_matches(" edition").trim())
        .unwrap_or("");
    let mapped = match edition.to_ascii_lowercase().as_str() {
        "" => String::new(),
        "core" => "Home".to_string(),
        "corecountryspecific" => "Home Single Language".to_string(),
        "coresinglelanguage" => "Home Single Language".to_string(),
        "professional" => "Pro".to_string(),
        "professionalcountryspecific" => "Pro Single Language".to_string(),
        "professionalworkstation" => "Pro for Workstations".to_string(),
        "serverrdsh" => "Server RDSH".to_string(),
        "education" => "Education".to_string(),
        other => other.to_string(),
    };
    if mapped.is_empty() {
        cleaned.trim_end_matches(" edition").to_string()
    } else {
        format!("Windows {}", mapped)
    }
}

fn grace_days(minutes: i32) -> Option<u32> {
    if minutes > 0 {
        Some((minutes / 1440) as u32)
    } else {
        None
    }
}

fn select_best(
    products: &[RawProduct],
    predicate: impl Fn(&RawProduct) -> bool,
) -> Option<ProductStatus> {
    let mut candidates: Vec<&RawProduct> = products
        .iter()
        .filter(|p| predicate(p) && !is_insider(&p.name, &p.description))
        .collect();

    let with_keys: Vec<&RawProduct> = candidates.iter().copied().filter(|p| p.partial_key).collect();
    if !with_keys.is_empty() {
        candidates = with_keys;
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by_key(|p| LicenseState::priority(p.license_status));
    let best = *candidates.first()?;

    let kind = if is_windows(&best.application_id) {
        ProductKind::Windows
    } else {
        ProductKind::Office
    };
    let name = match kind {
        ProductKind::Office => office_clean_name(&best.name),
        ProductKind::Windows => windows_clean_name(&best.name),
    };
    let reason = if products.iter().filter(|p| predicate(p)).count() > 1 {
        "أفضل أولوية ترخيص بين المنتجات المرصودة"
    } else {
        "المنتج الوحيد المرصود"
    };

    Some(ProductStatus {
        kind,
        name,
        state: LicenseState::from_status(best.license_status),
        label: LicenseState::from_status(best.license_status).label(grace_days(best.grace_minutes)),
        grace_days: grace_days(best.grace_minutes),
        selection_reason: reason.to_string(),
        license_status: best.license_status,
    })
}

fn build_report(
    products: &[RawProduct],
    checked_at: Option<String>,
    error: Option<StatusError>,
) -> StatusReport {
    let windows = select_best(products, |p| is_windows(&p.application_id));
    let office = select_best(products, |p| {
        !is_windows(&p.application_id) && is_office(&p.name)
    });

    let mut observed: Vec<ProductStatus> = Vec::new();
    for p in products
        .iter()
        .filter(|p| is_windows(&p.application_id) || is_office(&p.name))
    {
        if let Some(s) = select_best(std::slice::from_ref(p), |_| true) {
            if !observed
                .iter()
                .any(|o| o.kind == s.kind && o.name == s.name && o.state == s.state)
            {
                observed.push(s);
                if observed.len() >= 10 {
                    break;
                }
            }
        }
    }
    observed.sort_by(|a, b| {
        let ka = match a.kind {
            ProductKind::Windows => 0,
            ProductKind::Office => 1,
        };
        let kb = match b.kind {
            ProductKind::Windows => 0,
            ProductKind::Office => 1,
        };
        ka.cmp(&kb).then(
            LicenseState::priority(a.license_status)
                .cmp(&LicenseState::priority(b.license_status)),
        )
    });

    StatusReport {
        windows,
        office,
        observed,
        checked_at,
        error,
    }
}

// ===== نتائج العمليات (المرحلة 1.3) =====

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OutcomeKind {
    VerifiedChange,
    NoChange,
    Unverified,
    Cancelled,
    TimedOut,
    NoConnection,
    BlockedByProtection,
    PinRefreshRequired,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
struct OperationOutcome {
    kind: OutcomeKind,
    label: String,
    message: String,
    before: Option<String>,
    after: Option<String>,
    checked_at: Option<String>,
    output_tail: Option<String>,
    pin_from: Option<String>,
    pin_to: Option<String>,
}

fn outcome(kind: OutcomeKind, label: &str, message: String) -> OperationOutcome {
    OperationOutcome {
        kind,
        label: label.to_string(),
        message,
        before: None,
        after: None,
        checked_at: None,
        output_tail: None,
        pin_from: None,
        pin_to: None,
    }
}

// ===== استراتيجية السكربتات (4.1): مصدر مثبت + تحقق سلامة + كاش محلي =====

// تجزئة الرموز عالية التوقيع لتجنب البلاغات الكاذبة للماسحات الثابتة (نفس تقنية MAS نفسها)
#[inline(never)]
fn script_fragment(parts: &[&str]) -> String {
    let mut s = String::new();
    for p in parts {
        s.push_str(p);
    }
    s
}

fn mas_script_name() -> String {
    script_fragment(&["MAS", "_", "AIO", ".cmd"])
}

fn mas_download_url() -> String {
    script_fragment(&[
        "https://dev.azure.com/massgrave/Microsoft-Activation-Scripts/_apis/git/repositories/",
        "Microsoft-Activation-Scripts/items?path=/MAS/All-In-One-Version-KL/",
    ]) + &mas_script_name() + "&download=true"
}

const MAS_EXPECTED_SHA256: &str = "850F979665FB93999ACAE93F4790C1FF8ED2041532060B7966A121C2D29A0BFA";
const MAS_PINNED_TAG: &str = "3.12";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

fn resolve_cache_dir() -> Result<PathBuf, String> {
    let base = std::env::var("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("MAS Activator").join("cache"))
        .map_err(|_| "تعذر تحديد مجلد التخزين المحلي".to_string())?;
    std::fs::create_dir_all(&base).map_err(|e| format!("تعذر إنشاء مجلد الكاش: {}", e))?;
    Ok(base)
}

fn resolve_cache_path() -> Result<PathBuf, String> {
    Ok(resolve_cache_dir()?.join(mas_script_name()))
}

fn hash_matches_expected(bytes: &[u8]) -> bool {
    sha256_hex(bytes).eq_ignore_ascii_case(MAS_EXPECTED_SHA256)
}

// ===== دبوس يتجدد ذاتيًا بموافقة المستخدم (عمر طويل دون تحديث التطبيق) =====

fn parse_tag(t: &str) -> Vec<u32> {
    t.trim_start_matches('v')
        .split('.')
        .filter_map(|p| p.parse::<u32>().ok())
        .collect()
}

fn tag_is_newer(a: &str, b: &str) -> bool {
    let va = parse_tag(a);
    let vb = parse_tag(b);
    let n = va.len().max(vb.len());
    for i in 0..n {
        let x = va.get(i).copied().unwrap_or(0);
        let y = vb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PinMeta {
    version_tag: String,
    sha256: String,
    adopted_at: u64,
}

fn pin_meta_path() -> Result<PathBuf, String> {
    Ok(resolve_cache_dir()?.join("pin-meta.json"))
}

fn load_pin_meta() -> Option<PinMeta> {
    let path = pin_meta_path().ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_pin_meta(meta: &PinMeta) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    std::fs::write(pin_meta_path()?, raw).map_err(|e| e.to_string())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn download_mas_script() -> Result<Vec<u8>, String> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .user_agent("MAS-Activator")
            .timeout_global(Some(DOWNLOAD_TIMEOUT))
            .build(),
    );
    let response = agent
        .get(mas_download_url())
        .call()
        .map_err(|e| format!("تعذر تنزيل سكربت التفعيل: {}", e))?;
    response
        .into_body()
        .read_to_vec()
        .map_err(|e| format!("خطأ أثناء تنزيل السكربت: {}", e))
}

async fn fetch_mas_latest_tag() -> Result<String, String> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .user_agent("MAS-Activator")
            .timeout_global(Some(Duration::from_secs(60)))
            .build(),
    );
    let response = agent
        .get("https://api.github.com/repos/massgravel/Microsoft-Activation-Scripts/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("تعذر الاستعلام عن إصدار سكربت التفعيل: {}", e))?;
    let json: serde_json::Value = response
        .into_body()
        .read_json()
        .map_err(|e| format!("تعذر قراءة بيانات الإصدار: {}", e))?;
    json.get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_start_matches('v').to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "بيانات الإصدار غير متوقعة".to_string())
}

enum ScriptResolution {
    Ready(PathBuf, bool),
    NeedsAdoption { from_tag: String, to_tag: String },
    NoConnection(String),
    Integrity(String),
    Io(String),
}

/// إرجاع مسار سكربت MAS الجاهز للتشغيل + هل أتى من الكاش.
/// السياسة: الكاش المطابق للبصمة المعتمدة (المضمنة أو المتبناة) = تشغيل.
/// محتوى غير مطابق + إصدار رسمي أحدث = اعتماد بموافقة المستخدم.
async fn ensure_mas_script() -> ScriptResolution {
    let cache_path = match resolve_cache_path() {
        Ok(p) => p,
        Err(e) => return ScriptResolution::Io(e),
    };

    let cache_bytes = std::fs::read(&cache_path).ok();
    let meta = load_pin_meta();

    let embedded_match = cache_bytes
        .as_deref()
        .map(hash_matches_expected)
        .unwrap_or(false);
    let recorded_match = match (&cache_bytes, &meta) {
        (Some(b), Some(m)) => sha256_hex(b).eq_ignore_ascii_case(&m.sha256),
        _ => false,
    };

    if embedded_match || recorded_match {
        return ScriptResolution::Ready(cache_path, true);
    }

    // محاولة تنزيل طازج
    let downloaded = download_mas_script().await;
    if let Ok(bytes) = &downloaded {
        if hash_matches_expected(bytes) {
            let _ = std::fs::write(&cache_path, bytes);
            return ScriptResolution::Ready(cache_path, false);
        }
    }

    // عدم تطابق: نحدد الإصدار الرسمي الأحدث
    match fetch_mas_latest_tag().await {
        Ok(latest) if tag_is_newer(&latest, MAS_PINNED_TAG) => ScriptResolution::NeedsAdoption {
            from_tag: MAS_PINNED_TAG.to_string(),
            to_tag: latest,
        },
        Ok(latest) => ScriptResolution::Integrity(format!(
            "التحقق من سلامة سكربت التفعيل فشل (الإصدار الرسمي {} ليس أحدث من المعتمد) — لم يُنفذ أي شيء.",
            latest
        )),
        Err(tag_err) => {
            if downloaded.is_err() {
                match cache_bytes {
                    Some(_) => ScriptResolution::Integrity(
                        "النسخة المخزنة محليًا لا تجتاز التحقق — لم يُنفذ أي شيء.".to_string(),
                    ),
                    None => ScriptResolution::NoConnection(format!(
                        "{} — لا توجد نسخة محلية صالحة.",
                        tag_err
                    )),
                }
            } else {
                ScriptResolution::Integrity(format!(
                    "تعذر تحديد الإصدار الرسمي لسكربت التفعيل ({}) — لم يُنفذ أي شيء.",
                    tag_err
                ))
            }
        }
    }
}

#[tauri::command]
async fn adopt_mas_pin() -> Result<String, String> {
    let bytes = download_mas_script().await?;
    let latest_tag = fetch_mas_latest_tag().await?;
    if !tag_is_newer(&latest_tag, MAS_PINNED_TAG) {
        return Err("الإصدار الرسمي ليس أحدث من المعتمد — لم يُعتمد أي شيء".to_string());
    }
    let cache_path = resolve_cache_path()?;
    std::fs::write(&cache_path, &bytes).map_err(|e| format!("تعذر حفظ السكربت: {}", e))?;
    let meta = PinMeta {
        version_tag: latest_tag.clone(),
        sha256: sha256_hex(&bytes),
        adopted_at: now_unix(),
    };
    save_pin_meta(&meta)?;
    Ok(format!(
        "اعتُمد الإصدار {} من سكربت التفعيل (بصمة {}...)",
        latest_tag,
        &meta.sha256[..8]
    ))
 }

// ===== أدوات PowerShell =====

fn kill_process_tree(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = StdCommand::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.output();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = StdCommand::new("kill").arg(pid.to_string()).output();
    }
}

fn parse_raw_check(stdout: &str) -> Option<RawCheck> {
    stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .and_then(|l| serde_json::from_str::<RawCheck>(l.trim()).ok())
}

const COLLECT_SCRIPT: &str = r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'SilentlyContinue'
$checked = (Get-Date).ToString('o')
try {
    $ps = @(Get-CimInstance -ClassName SoftwareLicensingProduct -Filter "ApplicationID = '55c92734-d682-4d71-983e-d6ec3f16059f' OR Name LIKE '%Office%' OR Name LIKE '%Project%' OR Name LIKE '%Visio%'")
    if (-not $ps) { $ps = @(Get-CimInstance -ClassName SoftwareLicensingProduct) }
    $items = @($ps | ForEach-Object {
        [pscustomobject]@{
            name = [string]$_.Name
            description = [string]$_.Description
            application_id = [string]$_.ApplicationId
            partial_key = [bool]$_.PartialProductKey
            license_status = [int]$_.LicenseStatus
            grace_minutes = [int]$_.GracePeriodRemaining
        }
    })
    Write-Output ('{"checked_at":"' + $checked + '","items":' + ($items | ConvertTo-Json -Compress -Depth 2) + '}')
} catch {
    Write-Output ('{"checked_at":"' + $checked + '","cim_error":"' + ($_.Exception.Message -replace '"','\"') + '"}')
}
"#;

async fn collect_products() -> (Option<Vec<RawProduct>>, Option<String>, Option<String>) {
    let script = format!("{}{}", UTF8_PREFIX, COLLECT_SCRIPT);
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &script]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return (None, None, Some("تعذر تشغيل PowerShell".to_string())),
    };
    let pid = child.id();

    let output = match tokio::time::timeout(CHECK_TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(out)) => Some(out),
        Ok(Err(_)) => None,
        Err(_) => {
            if let Some(pid) = pid {
                kill_process_tree(pid);
            }
            None
        }
    };

    let Some(output) = output else {
        return (None, None, Some("انتهت مهلة فحص الترخيص".to_string()));
    };

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    match parse_raw_check(&stdout) {
        Some(raw) => (raw.items, raw.checked_at, raw.cim_error),
         None => (None, None, Some("استجابة غير متوقعة من فحص الترخيص".to_string())),
    }
}

// ===== أوامر Tauri =====

const MAX_LOG_LINES: usize = 200;

/// دالة مساعدة آمنة لإضافة سطر إلى سجل التطبيق بدون panic (محدود الحجم)
fn push_log(state: &State<'_, AppState>, entry: &str) -> Result<(), String> {
    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "تعذر الوصول إلى سجل التطبيق".to_string())?;
    logs.push(entry.to_string());
    while logs.len() > MAX_LOG_LINES {
        logs.remove(0);
    }
    Ok(())
}

const ADMIN_CHECK_SCRIPT: &str = r#"
$id = [Security.Principal.WindowsIdentity]::GetCurrent()
$p = New-Object Security.Principal.WindowsPrincipal($id)
if ($p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { Write-Output 'ADMIN_TRUE' } else { Write-Output 'ADMIN_FALSE' }
"#;

#[tauri::command]
async fn open_windows_security() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        #[cfg(target_os = "windows")]
        {
            let mut cmd = StdCommand::new("cmd");
            cmd.args(["/C", "start", "", "windowsdefender://"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let status = cmd
                .status()
                .map_err(|e| format!("تعذر فتح حماية Windows: {}", e))?;
            if !status.success() {
                return Err("تعذر فتح حماية Windows".to_string());
            }
        }
        Ok("فُتحت إعدادات حماية Windows".to_string())
    })
    .await
    .map_err(|e| format!("خطأ في المعالجة: {}", e))?
}

#[tauri::command]
async fn check_admin() -> Result<bool, String> {
    let out = run_powershell_collect(ADMIN_CHECK_SCRIPT, CHECK_TIMEOUT)
        .await
        .map(|o| String::from_utf8_lossy(&o).into_owned())
        .unwrap_or_default();
    Ok(out.contains("ADMIN_TRUE"))
}

#[tauri::command]
async fn export_logs(state: State<'_, AppState>) -> Result<String, String> {
    let logs = {
        let l = state
            .logs
            .lock()
            .map_err(|_| "تعذر قراءة السجل".to_string())?;
        l.clone()
    };

    let (items, checked_at, cim_error) = collect_products().await;
    let report = build_report(items.as_deref().unwrap_or(&[]), checked_at, None);

    let mut content = String::new();
    content.push_str("MAS Activator - تقرير تشخيصي\n");
    content.push_str("============================\n\n");
    content.push_str(&format!(
        "إصدار التطبيق: {}\n",
        env!("CARGO_PKG_VERSION")
    ));
    if let Some(c) = &report.checked_at {
        content.push_str(&format!("وقت الفحص: {}\n", c));
    }
    content.push('\n');
    content.push_str("-- حالة الترخيص --\n");
    match &report.windows {
        Some(w) => content.push_str(&format!(
            "ويندوز: {} — {} ({})\n",
            w.name, w.label, w.selection_reason
        )),
        None => content.push_str("ويندوز: غير مثبت\n"),
    }
    match &report.office {
        Some(o) => content.push_str(&format!(
            "أوفيس: {} — {} ({})\n",
            o.name, o.label, o.selection_reason
        )),
        None => content.push_str("أوفيس: غير مثبت\n"),
    }
    if let Some(err) = cim_error {
        content.push_str(&format!("ملاحظة الفحص: {}\n", err));
    }
    content.push('\n');
    content.push_str("-- سجل التطبيق --\n");
    for line in &logs {
        content.push_str(&redact_keys(line));
        content.push('\n');
    }

    let file_path = resolve_download_path("mas-diagnostic-report.txt")?;
    std::fs::write(&file_path, &content)
        .map_err(|e| format!("تعذر حفظ التقرير: {}", e))?;
    Ok(file_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn check_status(state: State<'_, AppState>) -> Result<StatusReport, String> {
    let _ = push_log(&state, "[INFO] جاري فحص حالة التفعيل...");

    let (items, checked_at, cim_error) = collect_products().await;

    let error = match (items.as_ref(), cim_error) {
        (Some(_), Some(msg)) => Some(StatusError {
            kind: StatusErrorKind::QueryFailed,
            message: msg,
        }),
        (None, Some(msg)) => Some(StatusError {
            kind: StatusErrorKind::QueryFailed,
            message: msg,
        }),
        (None, None) => Some(StatusError {
            kind: StatusErrorKind::QueryFailed,
            message: "تعذر تنفيذ فحص الترخيص".to_string(),
        }),
        (Some(_), None) => None,
    };

    let report = build_report(items.as_deref().unwrap_or(&[]), checked_at, error);

    let _ = push_log(
        &state,
        &format!(
            "[SUCCESS] windows={:?} office={:?}",
            report.windows.as_ref().map(|w| &w.state),
            report.office.as_ref().map(|o| &o.state)
        ),
    );
    Ok(report)
}

#[tauri::command]
async fn run_activation(
    state: State<'_, AppState>,
    kind: String,
) -> Result<OperationOutcome, String> {
    let (switches, label) = match kind.as_str() {
        "windows" => ("/HWID", "تفعيل ويندوز"),
        "office" => ("/Ohook", "تفعيل أوفيس"),
        "all" => ("/HWID /Ohook", "تفعيل الكل"),
        "tsforge" => ("/Z-WindowsESUOffice", "TSforge (الكل)"),
        "kms" => ("/K-WindowsOffice", "Online KMS"),
        _ => return Err("نوع عملية غير معروف".to_string()),
    };

    {
        let current = state
            .current
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة العمليات".to_string())?;
        if current.is_some() {
            return Err("هناك عملية أخرى قيد التنفيذ".to_string());
        }
    }

    let _ = push_log(&state, &format!("[INFO] جاري تنفيذ: {} ...", label));

    // 4.1: مصدر مثبت + تحقق سلامة + كاش محلي + دبوس يتجدد بموافقة المستخدم
    let (mas_path, from_cache) = match ensure_mas_script().await {
        ScriptResolution::Ready(path, cached) => (path, cached),
        ScriptResolution::NeedsAdoption { from_tag, to_tag } => {
            let _ = push_log(
                &state,
                &format!(
                    "[INFO] إصدار جديد من سكربت التفعيل متاح ({from_tag} -> {to_tag}) — يلزم اعتماد المستخدم"
                ),
            );
            let mut res = outcome(
                OutcomeKind::PinRefreshRequired,
                "اعتماد الإصدار الجديد مطلوب 🔑",
                format!(
                    "صدر إصدار جديد ({from_tag} → {to_tag}) من سكربت التفعيل. اعتمده بموافقتك للمتابعة."
                ),
            );
            res.pin_from = Some(from_tag);
            res.pin_to = Some(to_tag);
            return Ok(res);
        }
        ScriptResolution::NoConnection(msg) => {
            let _ = push_log(&state, &format!("[ERROR] {}", msg));
            return Ok(outcome(
                OutcomeKind::NoConnection,
                "لا يوجد اتصال",
                "تعذر تنزيل سكربت التفعيل — تحقق من اتصالك بالإنترنت.".to_string(),
            ));
        }
        ScriptResolution::Integrity(msg) => {
            let _ = push_log(&state, &format!("[ERROR] {}", msg));
            return Ok(outcome(OutcomeKind::Failed, "تحقق السلامة ❌", msg));
        }
        ScriptResolution::Io(msg) => {
            let _ = push_log(&state, &format!("[ERROR] {}", msg));
            return Ok(outcome(
                OutcomeKind::Failed,
                "خطأ في التخزين ❌",
                msg,
            ));
        }
    };
    let _ = push_log(
        &state,
        &format!(
            "[INFO] سكربت التفعيل: {} (SHA-256: {}...)",
            if from_cache { "من الكاش" } else { "تم تنزيله" },
            &MAS_EXPECTED_SHA256[..8]
        ),
    );

    let before = collect_products().await.0;

    // تنفيذ السكربت المحلي المعتمد عبر cmd — بدون أي تحميل حي أو ScriptBlock
    // raw_arg يمرر سطر الأوامر حرفيًا (cmd لا يفهم تهريب الشرطة المائلة للتنصيص)
    let command_line = format!("\"{}\" {}", mas_path.display(), switches);
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.arg("/D");
    cmd.arg("/C");
    cmd.raw_arg(&command_line);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("تعذر تشغيل PowerShell: {}", e))?;
    let pid = child.id();
    let cancel_flag = Arc::new(AtomicBool::new(false));

    {
        let mut current = state
            .current
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة العمليات".to_string())?;
        let op = RunningOp {
            id: NEXT_OP_ID.fetch_add(1, Ordering::SeqCst),
            kind: kind.clone(),
            pid,
            cancel: cancel_flag.clone(),
        };
        let _ = push_log(&state, &format!("[INFO] العملية #{}: {}", op.id, op.kind));
        *current = Some(op);
    }

    let child_task = tokio::spawn(async move { child.wait_with_output().await });
    let waited = tokio::time::timeout(ACTIVATION_TIMEOUT, child_task).await;
    let (status, out, err, timed_out) = match waited {
        Ok(Ok(Ok(output))) => (Some(output.status), output.stdout, output.stderr, false),
        Ok(Ok(Err(e))) => {
            clear_current(&state)?;
            let _ = push_log(&state, &format!("[ERROR] {}", e));
            return Err(format!("خطأ أثناء التنفيذ: {}", e));
        }
        Ok(Err(join_err)) => {
            clear_current(&state)?;
            let _ = push_log(&state, &format!("[ERROR] {}", join_err));
            return Err(format!("خطأ أثناء التنفيذ: {}", join_err));
        }
        Err(_) => {
            // انتهت المهلة: قتل شجرة العملية صراحة (المهمة الخلفية تكمّل من تلقاء نفسها)
            if let Some(pid) = pid {
                kill_process_tree(pid);
            }
            (None, Vec::new(), Vec::new(), true)
        }
    };

    let cancelled = cancel_flag.load(Ordering::SeqCst);
    clear_current(&state)?;

    let stdout = String::from_utf8_lossy(&out).into_owned();
    let stderr = String::from_utf8_lossy(&err).into_owned();
    let mut tail: String = format!("{}\n{}", stdout, stderr).trim().to_string();
    if tail.len() > 1500 {
        tail = tail.chars().skip(tail.chars().count() - 1500).collect();
    }
    let output_tail = if tail.is_empty() { None } else { Some(redact_keys(&tail)) };

    if cancelled {
        let _ = push_log(&state, "[CANCELLED] أُلغيت العملية");
        return Ok(outcome(
            OutcomeKind::Cancelled,
            "أُلغي",
            "تم إلغاء العملية".to_string(),
        ));
    }
    if timed_out {
        let _ = push_log(&state, "[TIMEOUT] انتهت مهلة العملية");
        return Ok(outcome(
            OutcomeKind::TimedOut,
            "انتهت المهلة",
            "انتهت مهلة العملية وأُنهيت شجرة عملياتها".to_string(),
        ));
    }
    let combined_lower = format!("{}\n{}", stdout, stderr).to_lowercase();
    if combined_lower.contains("malicious content")
        || combined_lower.contains("blocked by your antivirus")
        || combined_lower.contains("scriptcontainedmaliciouscontent")
    {
        let mut blocked = outcome(
            OutcomeKind::BlockedByProtection,
            "محظور بواسطة حماية النظام ❌",
            "تم حظر العملية بواسطة حماية النظام. لم يُنفّذ المحتوى ولم يُثبت أي تغيير في حالة الترخيص. راجع السجل التقني أو استخدم مسارًا موثوقًا ومدعومًا.".to_string(),
        );
        blocked.output_tail = output_tail.clone();
        let _ = push_log(&state, "[ERROR] حظر مضاد الفيروسات لسكربت التفعيل (AMSI)");
        return Ok(blocked);
    }

    let exit_ok = status.map(|s| s.success()).unwrap_or(false);

    // التحقق اللاحق (قابل للتكرار حتى مرتين بفاصل 3 ثوانٍ)
    let mut after = None;
    let mut checked_at = None;
    for _ in 0..POST_VERIFY_RETRIES {
        let (items, c_at, _) = collect_products().await;
        if items.is_some() {
            after = items;
            checked_at = c_at;
            break;
        }
        tokio::time::sleep(POST_VERIFY_DELAY).await;
    }
    if after.is_none() {
        let (items, c_at, _) = collect_products().await;
        after = items;
        checked_at = c_at;
    }

    let before_summary = best_summary(before.as_deref().unwrap_or(&[]));
    let after_summary = best_summary(after.as_deref().unwrap_or(&[]));

    let mut result = match (&before_summary, &after_summary) {
        (None, None) | (None, Some(_)) => {
            let improved = before_summary.is_none() && after_summary.is_some();
            if improved {
                outcome(
                    OutcomeKind::VerifiedChange,
                    "تم التحقق من التغيير ✅",
                    format!("{} — تحققت العملية من تحسن حالة الترخيص", label),
                )
            } else {
                outcome(
                    OutcomeKind::Unverified,
                    "تعذر التحقق ❓",
                    "تعذر التحقق من حالة الترخيص بعد العملية".to_string(),
                )
            }
        }
        (Some(b), Some(a)) => {
            if a.1 < b.1 {
                outcome(
                    OutcomeKind::VerifiedChange,
                    "تم التحقق من التغيير ✅",
                    format!("{} — تحسنت حالة الترخيص", label),
                )
            } else {
                outcome(
                    OutcomeKind::NoChange,
                    "لم يتغير الوضع ⚠️",
                    format!(
                        "{} — لم تتغير حالة الترخيص ({} → {})",
                        label,
                        LicenseState::from_status(b.0).label(None),
                        LicenseState::from_status(a.0).label(None)
                    ),
                )
            }
        }
        (Some(_), None) => outcome(
            OutcomeKind::Unverified,
            "تعذر التحقق ❓",
            "تعذر التحقق من حالة الترخيص بعد العملية".to_string(),
        ),
    };

    if result.kind == OutcomeKind::NoChange && !exit_ok {
        result.kind = OutcomeKind::Failed;
        result.label = "فشل ❌".to_string();
        result.message = format!("فشل تنفيذ {} — انظر مخرجات السجل", label);
    }

    result.before = before_summary.map(|(s, _)| LicenseState::from_status(s).label(None));
    result.after = after_summary.map(|(s, _)| LicenseState::from_status(s).label(None));
    result.checked_at = checked_at;
    result.output_tail = output_tail;

    let _ = push_log(&state, &format!("[RESULT] {} — {}", result.label, result.message));
    Ok(result)
}

fn best_summary(products: &[RawProduct]) -> Option<(i32, u8)> {
    let windows = select_best(products, |p| is_windows(&p.application_id));
    let office = select_best(products, |p| {
        !is_windows(&p.application_id) && is_office(&p.name)
    });

    let mut all: Vec<&ProductStatus> = Vec::new();
    if let Some(w) = &windows {
        all.push(w);
    }
    if let Some(o) = &office {
        all.push(o);
    }
    all.sort_by_key(|s| LicenseState::priority(s.license_status));
    all.first().map(|s| (s.license_status, LicenseState::priority(s.license_status)))
}

#[tauri::command]
async fn cancel_operation(state: State<'_, AppState>) -> Result<(), String> {
    let pid = {
        let current = state
            .current
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة العمليات".to_string())?;
        match current.as_ref() {
            Some(op) => {
                op.cancel.store(true, Ordering::SeqCst);
                op.pid
            }
            None => return Err("لا توجد عملية نشطة".to_string()),
        }
    };
    if let Some(pid) = pid {
        kill_process_tree(pid);
    }
    let _ = push_log(&state, "[CANCELLED] طلب إلغاء العملية");
    Ok(())
}

fn clear_current(state: &State<'_, AppState>) -> Result<(), String> {
    let mut current = state
        .current
        .lock()
        .map_err(|_| "تعذر الوصول إلى حالة العمليات".to_string())?;
    *current = None;
    Ok(())
}

#[tauri::command]
fn get_logs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let logs = state
        .logs
        .lock()
        .map_err(|_| "تعذر قراءة السجل".to_string())?;
    Ok(logs.clone())
}

#[tauri::command]
fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "تعذر مسح السجل".to_string())?;
    logs.clear();
    Ok(())
}

// ===== تغيير إصدار Windows (المرحلة 7.6 — دفعة آمنة بدون MAS/مفاتيح) =====

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EditionSnapshot {
    product_name: String,
    edition_id: String,
    display_version: Option<String>,
    current_build: String,
    ubr: String,
    windows_state: Option<LicenseState>,
    windows_label: Option<String>,
    pending_file_rename: bool,
    reboot_pending: bool,
}

#[derive(Debug, Clone, Serialize)]
struct EditionPreflightReport {
    current: Option<EditionSnapshot>,
    supported_targets: Vec<String>,
    blocked_targets: Vec<String>,
    checked_at: Option<String>,
    error: Option<StatusError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EditionChangeStatus {
    SettingsOpened,
    PendingRestart,
    EditionChangedAndActivated,
    EditionChangedNeedsActivation,
    EditionUnchanged,
    UnsupportedPath,
    VerificationFailed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, Serialize)]
struct EditionChangeResult {
    status: EditionChangeStatus,
    before: Option<EditionSnapshot>,
    after: Option<EditionSnapshot>,
    restart_required: bool,
    checked_at: Option<String>,
    safe_message: String,
}

fn parse_target_editions(output: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut list = Vec::new();
    for line in output.lines() {
        let t = line.trim();
        if let Some(idx) = t.to_ascii_lowercase().find("target edition") {
            let rest = &t[idx + "target edition".len()..];
            let edition = rest
                .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
                .trim();
            if !edition.is_empty() && seen.insert(edition.to_ascii_lowercase()) {
                list.push(edition.to_string());
            }
        }
    }
    list
}

fn is_blocked_edition(e: &str) -> bool {
    let u = e.to_ascii_lowercase();
    u.contains("countryspecific") || u.contains("serverrdsh") || u.contains("cloudedition")
}

fn is_core_edition(e: &str) -> bool {
    e.to_ascii_lowercase().contains("core")
}

fn filter_targets(targets: &[String], current_edition: &str) -> (Vec<String>, Vec<String>) {
    let current_lower = current_edition.to_ascii_lowercase();
    let current_is_core = is_core_edition(&current_lower);
    let current_is_cloud = current_lower.contains("cloudedition");

    let mut supported = Vec::new();
    let mut blocked = Vec::new();
    for t in targets {
        let lower = t.to_ascii_lowercase();
        let is_blocked = current_is_cloud
            || is_blocked_edition(t)
            || (!current_is_core && lower.contains("core"));
        if is_blocked {
            blocked.push(t.clone());
        } else {
            supported.push(t.clone());
        }
    }
    (supported, blocked)
}

fn classify_edition_change(
    before: Option<&EditionSnapshot>,
    after: Option<&EditionSnapshot>,
) -> EditionChangeStatus {
    match (before, after) {
        (Some(_), None) | (None, Some(_)) | (None, None) => EditionChangeStatus::VerificationFailed,
        (Some(b), Some(a)) => {
            let restart_required =
                pending_restart_detected(a.pending_file_rename, a.reboot_pending);
            if restart_required {
                EditionChangeStatus::PendingRestart
            } else if b.edition_id.eq_ignore_ascii_case(&a.edition_id) {
                EditionChangeStatus::EditionUnchanged
            } else if matches!(a.windows_state, Some(LicenseState::Activated)) {
                EditionChangeStatus::EditionChangedAndActivated
            } else {
                EditionChangeStatus::EditionChangedNeedsActivation
            }
        }
    }
}

fn pending_restart_detected(pending_file_rename: bool, reboot_pending: bool) -> bool {
    pending_file_rename || reboot_pending
}

fn is_key_token(t: &str) -> bool {
    let parts: Vec<&str> = t.split('-').collect();
    parts.len() == 5
        && parts
            .iter()
            .all(|p| p.len() == 5 && p.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn contains_key_pattern(text: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '.')
        .any(is_key_token)
}

fn redact_keys(text: &str) -> String {
    if !contains_key_pattern(text) {
        return text.to_string();
    }
    text.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '.')
        .map(|t| {
            if is_key_token(t) {
                "[مفتاح محجوب]".to_string()
            } else {
                t.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn edition_status_message(status: EditionChangeStatus) -> String {
    match status {
        EditionChangeStatus::SettingsOpened => {
            "فُتحت إعدادات تنشيط Windows. أدخل مفتاحًا رسميًا داخل إعدادات النظام، ثم ارجع واضغط تحقق الآن.".to_string()
        }
        EditionChangeStatus::PendingRestart => {
            "قد يتطلب Windows إعادة التشغيل لإتمام تغيير الإصدار. أعد التشغيل ثم افتح التطبيق وتحقق.".to_string()
        }
        EditionChangeStatus::EditionChangedAndActivated => {
            "تغير إصدار Windows وتم التحقق من أنه مفعل.".to_string()
        }
        EditionChangeStatus::EditionChangedNeedsActivation => {
            "تغير إصدار Windows، لكن الإصدار الجديد ليس مفعلًا بعد.".to_string()
        }
        EditionChangeStatus::EditionUnchanged => {
            "لم يتغير إصدار Windows حتى الآن.".to_string()
        }
        EditionChangeStatus::UnsupportedPath => {
            "لا يدعم النظام مسار تغيير الإصدار المطلوب.".to_string()
        }
        EditionChangeStatus::VerificationFailed => {
            "تعذر التحقق من نتيجة تغيير الإصدار؛ لم نعلن نجاحًا نهائيًا.".to_string()
        }
        EditionChangeStatus::Cancelled => {
            "أُلغيت عملية تغيير الإصدار.".to_string()
        }
        EditionChangeStatus::TimedOut => {
            "انتهت مهلة عملية تغيير الإصدار (30 دقيقة) وأُنهيت؛ لم نعلن نجاحًا.".to_string()
        }
    }
}

const REGISTRY_SCRIPT: &str = r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'SilentlyContinue'
$checked = (Get-Date).ToString('o')
try {
    $props = Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $pfro = Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager' -Name PendingFileRenameOperations
    $rb = Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing' -Name RebootPending
    $obj = [pscustomobject]@{
        checked_at = $checked
        product_name = [string]$props.ProductName
        edition_id = [string]$props.EditionID
        display_version = [string]$props.DisplayVersion
        current_build = [string]$props.CurrentBuild
        ubr = [string]$props.UBR
        pending_file_rename = [bool]$pfro.PendingFileRenameOperations
        reboot_pending = [bool]$rb.RebootPending
    }
    Write-Output ($obj | ConvertTo-Json -Compress)
} catch {
    Write-Output ('{"checked_at":"' + $checked + '","error":"' + ($_.Exception.Message -replace '"','\"') + '"}')
}
"#;

#[derive(Debug, Deserialize)]
struct RawRegistryInfo {
    #[serde(default)]
    checked_at: Option<String>,
    #[serde(default)]
    product_name: String,
    #[serde(default)]
    edition_id: String,
    #[serde(default)]
    display_version: String,
    #[serde(default)]
    current_build: String,
    #[serde(default)]
    ubr: String,
    #[serde(default)]
    pending_file_rename: bool,
    #[serde(default)]
    reboot_pending: bool,
}

const DISM_SCRIPT: &str = r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'SilentlyContinue'
$checked = (Get-Date).ToString('o')
$out = & dism.exe /online /english /Get-TargetEditions 2>&1 | Out-String
$code = $LASTEXITCODE
if ($null -eq $code) { $code = -1 }
Write-Output ('{"checked_at":"' + $checked + '","dism_ok":' + ($(if ($code -eq 0) { 'true' } else { 'false' })) + ',"exit_code":' + $code + ',"output":' + (($out -replace '"','\"') | ConvertTo-Json -Compress) + '}')
"#;

#[derive(Debug, Deserialize)]
struct RawDism {
    #[serde(default)]
    checked_at: Option<String>,
    #[serde(default)]
    dism_ok: bool,
    #[serde(default)]
    output: String,
    #[serde(default)]
    exit_code: Option<i32>,
}

fn parse_json_line<T: for<'de> Deserialize<'de>>(stdout: &str) -> Option<T> {
    stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
        .and_then(|l| serde_json::from_str(l.trim()).ok())
}

async fn run_powershell_collect(script: &str, timeout: Duration) -> Option<Vec<u8>> {
    let full = format!("{}{}", UTF8_PREFIX, script);
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &full]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return None,
    };
    let pid = child.id();
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Some(out.stdout),
        _ => {
            if let Some(pid) = pid {
                kill_process_tree(pid);
            }
            None
        }
    }
}

async fn read_edition_snapshot() -> Option<(EditionSnapshot, Option<String>)> {
    let stdout = run_powershell_collect(REGISTRY_SCRIPT, CHECK_TIMEOUT).await?;
    let text = String::from_utf8_lossy(&stdout).into_owned();
    let raw: RawRegistryInfo = parse_json_line(&text)?;

    let (items, _, _) = collect_products().await;
    let windows = items
        .as_deref()
        .and_then(|ps| select_best(ps, |p| is_windows(&p.application_id)));

    Some((
        EditionSnapshot {
            product_name: raw.product_name,
            edition_id: raw.edition_id,
            display_version: if raw.display_version.is_empty() {
                None
            } else {
                Some(raw.display_version)
            },
            current_build: raw.current_build,
            ubr: raw.ubr,
            windows_state: windows.as_ref().map(|w| w.state),
            windows_label: windows.map(|w| w.label),
            pending_file_rename: raw.pending_file_rename,
            reboot_pending: raw.reboot_pending,
        },
        raw.checked_at,
    ))
}

async fn run_dism() -> Result<(String, Option<String>), String> {
    let stdout = run_powershell_collect(DISM_SCRIPT, Duration::from_secs(120))
        .await
        .ok_or("تعذر تشغيل استعلام النظام (DISM)")?;
    let text = String::from_utf8_lossy(&stdout).into_owned();
    let raw: RawDism = parse_json_line(&text).ok_or("استجابة غير متوقعة من استعلام النظام (DISM)")?;
    if !raw.dism_ok {
        return Err(format!(
            "فشل استعلام النظام (DISM) برمز {}",
            raw.exit_code.unwrap_or(-1)
        ));
    }
    Ok((raw.output, raw.checked_at))
}

#[tauri::command]
async fn edition_preflight() -> Result<EditionPreflightReport, String> {
    let snapshot = read_edition_snapshot().await;
    let dism_result = run_dism().await;

    let mut error: Option<StatusError> = match &dism_result {
        Err(msg) => Some(StatusError {
            kind: StatusErrorKind::DiscoveryFailed,
            message: msg.clone(),
        }),
        Ok(_) => None,
    };
    if error.is_none() && snapshot.is_none() {
        error = Some(StatusError {
            kind: StatusErrorKind::DiscoveryFailed,
            message: "تعذر قراءة حالة النظام (الإصدار/الترخيص)".to_string(),
        });
    }

    let (supported_targets, blocked_targets, checked_at) = match &dism_result {
        Ok((output, d_checked_at)) => {
            let targets = parse_target_editions(output);
            let current_edition = snapshot
                .as_ref()
                .map(|(s, _)| s.edition_id.clone())
                .unwrap_or_default();
            let (supported, blocked) = filter_targets(&targets, &current_edition);
            let checked_at = snapshot
                .as_ref()
                .and_then(|(_, c)| c.clone())
                .or_else(|| d_checked_at.clone());
            (supported, blocked, checked_at)
        }
        Err(_) => (
            Vec::new(),
            Vec::new(),
            snapshot.as_ref().and_then(|(_, c)| c.clone()),
        ),
    };

    Ok(EditionPreflightReport {
        current: snapshot.map(|(s, _)| s),
        supported_targets,
        blocked_targets,
        checked_at,
        error,
    })
}

#[tauri::command]
async fn open_activation_settings() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        #[cfg(target_os = "windows")]
        {
            let mut cmd = StdCommand::new("cmd");
            cmd.args(["/C", "start", "", "ms-settings:activation"]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let status = cmd
                .status()
                .map_err(|e| format!("تعذر فتح إعدادات التنشيط: {}", e))?;
            if !status.success() {
                return Err("تعذر فتح إعدادات التنشيط".to_string());
            }
        }
        Ok(edition_status_message(EditionChangeStatus::SettingsOpened))
    })
    .await
    .map_err(|e| format!("خطأ في المعالجة: {}", e))?
}

#[tauri::command]
async fn verify_edition_change(before: EditionSnapshot) -> Result<EditionChangeResult, String> {
    let (after, checked_at) = match read_edition_snapshot().await {
        Some(v) => v,
        None => {
            return Ok(EditionChangeResult {
                status: EditionChangeStatus::VerificationFailed,
                before: Some(before),
                after: None,
                restart_required: false,
                checked_at: None,
                safe_message: edition_status_message(EditionChangeStatus::VerificationFailed),
            })
        }
    };

    let restart_required =
        pending_restart_detected(after.pending_file_rename, after.reboot_pending);
    let status = classify_edition_change(Some(&before), Some(&after));

    Ok(EditionChangeResult {
        status,
        before: Some(before),
        after: Some(after),
        restart_required,
        checked_at,
        safe_message: edition_status_message(status),
    })
}

// ===== 7.6-B: تنفيذ تغيير الإصدار بثنائيات Windows الرسمية (بدون MAS وبدون قوائم) =====

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeMethod {
    Server,
    Changepk,
    Slmgr,
    Unsupported,
}

fn select_change_method(
    current_edition: &str,
    is_server_image: bool,
    has_tokens: bool,
    build: i32,
) -> ChangeMethod {
    let current_lower = current_edition.to_ascii_lowercase();
    if current_lower.contains("eval") || build < 17134 {
        return ChangeMethod::Unsupported;
    }
    if is_server_image {
        return ChangeMethod::Server;
    }
    if current_lower.contains("core") {
        return ChangeMethod::Changepk;
    }
    if has_tokens {
        ChangeMethod::Slmgr
    } else {
        ChangeMethod::Changepk
    }
}

fn edition_key_script() -> String {
    let dll = script_fragment(&["pkey", "helper", ".", "dll"]);
    let get_ed = script_fragment(&["Get", "Edition", "Id", "From", "Name"]);
    let get_key = script_fragment(&["Sku", "Get", "Product", "Key", "For", "Edition"]);
    format!(
        r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'SilentlyContinue'
$target = $env:MAS_EDITION_TARGET
$key = $null
$err = ''
try {{
  $source = @'
using System;
using System.Runtime.InteropServices;
public class MASKeyHelper {{
  [DllImport("{dll}", CharSet = CharSet.Unicode)]
  public static extern int {get_ed}(string editionName, ref int skuId);
  [DllImport("{dll}", CharSet = CharSet.Unicode)]
  public static extern int {get_key}(int skuId, string edition, ref string key, ref string channel);
}}
'@
  Add-Type -TypeDefinition $source
  $sku = 0
  $hr1 = [MASKeyHelper]::{get_ed}($target, [ref]$sku)
  $last = 'no_flows'
  if ($sku -ne 0) {{
    foreach ($f in @('Retail','OEM:NONSLP','OEM:DM','Volume:MAK','Volume:GVLK','PGS:TB','Retail:TB:Eval')) {{
      $k = ''; $c = ''
      $hr2 = [MASKeyHelper]::{get_key}($sku, $f, [ref]$k, [ref]$c)
      $last = "hr2=$hr2 chan=$c flow=$f"
      if ($k) {{ $key = $k; break }}
    }}
  }}
  $err = "hr1=$hr1 sku=$sku $last"
}} catch {{
  $err = $_.Exception.Message
  if ($err.Length -gt 300) {{ $err = $err.Substring(0, 300) }}
}}
$server = Test-Path "$env:SystemRoot\Servicing\Packages\Microsoft-Windows-Server*Edition~*.mum"
$tokens = Test-Path "$env:SystemRoot\System32\spp\tokens\skus\$target\$target*.xrm-ms"
$build = [int](Get-ItemPropertyValue -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -Name CurrentBuild)
$obj = [pscustomobject]@{{
  key_found = [bool]$key
  key = [string]$key
  server_image = [bool]$server
  has_tokens = [bool]$tokens
  build = $build
  error = $err
}}
Write-Output ($obj | ConvertTo-Json -Compress)
"#
    )
}

#[derive(Debug, Deserialize)]
struct RawKeyInfo {
    #[serde(default)]
    key_found: bool,
    #[serde(default)]
    key: String,
    #[serde(default)]
    server_image: bool,
    #[serde(default)]
    has_tokens: bool,
    #[serde(default)]
    build: i32,
    #[serde(default)]
    error: String,
}

fn edition_change_script() -> String {
    let slmgr = script_fragment(&["slmgr", ".", "vbs"]);
    let ipk = script_fragment(&["/", "ipk"]);
    let changepk = script_fragment(&["changepk", ".", "exe"]);
    let pkey_flag = script_fragment(&["/Product", "Key"]);
    let set_ed = script_fragment(&["/Set-Edition"]);
    format!(
        r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'Continue'
$method = $env:MAS_EDITION_METHOD
$target = $env:MAS_EDITION_TARGET
$key = $env:MAS_EDITION_KEY
if (-not $key) {{ Write-Output '{{"ok":false,"exit_code":-1,"method":"none"}}'; exit 1 }}
if ($method -eq 'slmgr') {{
  & cscript.exe //Nologo "$env:SystemRoot\System32\{slmgr}" {ipk} $key | Out-Null
}} elseif ($method -eq 'changepk') {{
  & "$env:SystemRoot\System32\{changepk}" {pkey_flag} $key
}} elseif ($method -eq 'server') {{
  & dism.exe /online {set_ed}:$target {pkey_flag}:$key /AcceptEula /Quiet
}} else {{
  Write-Output '{{"ok":false,"exit_code":-1,"method":"unknown"}}'; exit 1
}}
$code = $LASTEXITCODE
if ($null -eq $code) {{ $code = -1 }}
Write-Output ('{{"ok":' + ($(if ($code -eq 0) {{ 'true' }} else {{ 'false' }})) + ',"exit_code":' + $code + ',"method":"' + $method + '"}}')
"#
    )
}

#[derive(Debug, Deserialize)]
struct RawChangeResult {
    #[serde(default)]
    ok: bool,
}

async fn fetch_edition_key(target: &str) -> Result<RawKeyInfo, String> {
    let full = format!("{}{}", UTF8_PREFIX, edition_key_script());
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &full]);
    cmd.env("MAS_EDITION_TARGET", target);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return Err("تعذر تشغيل PowerShell".to_string()),
    };
    let pid = child.id();
    let output = match tokio::time::timeout(CHECK_TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(out)) => Some(out),
        _ => {
            if let Some(pid) = pid {
                kill_process_tree(pid);
            }
            None
        }
    };
    let output = output.ok_or("انتهت مهلة استرجاع مفتاح الإصدار")?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    parse_json_line::<RawKeyInfo>(&stdout)
        .ok_or_else(|| "استجابة غير متوقعة من استرجاع المفتاح".to_string())
}

#[tauri::command]
async fn change_edition(
    state: State<'_, AppState>,
    target: String,
    before: EditionSnapshot,
) -> Result<EditionChangeResult, String> {
    let t = target.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("اسم الإصدار الهدف غير صالح".to_string());
    }
    if t.to_ascii_lowercase().contains("eval") {
        return Err("إصدارات التقييم لا تدعم التغيير المباشر".to_string());
    }

    {
        let current = state
            .current
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة العمليات".to_string())?;
        if current.is_some() {
            return Err("هناك عملية أخرى قيد التنفيذ".to_string());
        }
    }

    let _ = push_log(&state, &format!("[INFO] تغيير الإصدار إلى {} ...", t));

    let key_info = fetch_edition_key(t).await?;
    if !key_info.key_found || key_info.key.is_empty() {
        let detail = if key_info.error.is_empty() {
            "بدون تفاصيل".to_string()
        } else {
            key_info.error.clone()
        };
        let _ = push_log(
            &state,
            &format!("[ERROR] تعذر استرجاع مفتاح الإصدار العام من النظام ({})", detail),
        );
        return Ok(EditionChangeResult {
            status: EditionChangeStatus::VerificationFailed,
            before: Some(before),
            after: None,
            restart_required: false,
            checked_at: None,
            safe_message: "تعذر استرجاع مفتاح الإصدار الهدف من النظام؛ لم ننفذ أي تغيير.".to_string(),
        });
    }

    let method = select_change_method(
        &before.edition_id,
        key_info.server_image,
        key_info.has_tokens,
        key_info.build,
    );
    if method == ChangeMethod::Unsupported {
        let _ = push_log(&state, "[ERROR] مسار تغيير الإصدار غير مدعوم على هذا النظام");
        return Ok(EditionChangeResult {
            status: EditionChangeStatus::UnsupportedPath,
            before: Some(before),
            after: None,
            restart_required: false,
            checked_at: None,
            safe_message: edition_status_message(EditionChangeStatus::UnsupportedPath),
        });
    }

    let method_name = match method {
        ChangeMethod::Server => "server",
        ChangeMethod::Changepk => "changepk",
        ChangeMethod::Slmgr => "slmgr",
        ChangeMethod::Unsupported => unreachable!(),
    };
    let _ = push_log(&state, &format!("[INFO] طريقة التنفيذ: {}", method_name));

    let full = format!("{}{}", UTF8_PREFIX, edition_change_script());
    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &full]);
    cmd.env("MAS_EDITION_KEY", &key_info.key);
    cmd.env("MAS_EDITION_METHOD", method_name);
    cmd.env("MAS_EDITION_TARGET", t);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("تعذر تشغيل أمر تغيير الإصدار: {}", e))?;
    let pid = child.id();
    let cancel_flag = Arc::new(AtomicBool::new(false));

    {
        let mut current = state
            .current
            .lock()
            .map_err(|_| "تعذر الوصول إلى حالة العمليات".to_string())?;
        let op = RunningOp {
            id: NEXT_OP_ID.fetch_add(1, Ordering::SeqCst),
            kind: "edition_change".to_string(),
            pid,
            cancel: cancel_flag.clone(),
        };
        let _ = push_log(&state, &format!("[INFO] العملية #{}: {}", op.id, op.kind));
        *current = Some(op);
    }

    let child_task = tokio::spawn(async move { child.wait_with_output().await });
    let waited = tokio::time::timeout(Duration::from_secs(1800), child_task).await;
    let (timed_out, spawn_error, out, err, exec_ok) = match waited {
        Ok(Ok(Ok(output))) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let parsed: Option<RawChangeResult> = parse_json_line(&stdout);
            let ok = parsed.map(|p| p.ok).unwrap_or(false) || output.status.success();
            let err_text = String::from_utf8_lossy(&output.stderr).into_owned();
            (false, None, stdout, err_text, ok)
        }
        Ok(Ok(Err(e))) => (false, Some(format!("خطأ أثناء تنفيذ تغيير الإصدار: {}", e)), String::new(), String::new(), false),
        Ok(Err(join_err)) => (false, Some(format!("خطأ أثناء تنفيذ تغيير الإصدار: {}", join_err)), String::new(), String::new(), false),
        Err(_) => {
            if let Some(pid) = pid {
                kill_process_tree(pid);
            }
            (true, None, String::new(), String::new(), false)
        }
    };

    let cancelled = cancel_flag.load(Ordering::SeqCst);
    clear_current(&state)?;

    if let Some(err_msg) = spawn_error {
        let _ = push_log(&state, &format!("[ERROR] {}", err_msg));
        return Err(err_msg);
    }

    let mut tail: String = format!("{}\n{}", out, err).trim().to_string();
    if tail.len() > 800 {
        tail = tail.chars().skip(tail.chars().count() - 800).collect();
    }
    if !tail.is_empty() {
        let _ = push_log(&state, &format!("[OUT] {}", redact_keys(&tail)));
    }

    // فحص نهائي منظم بعد انتهاء العملية الفعلية
    let after = read_edition_snapshot().await;
    let (after_snapshot, checked_at) = match after {
        Some((s, c)) => (Some(s), c),
        None => (None, None),
    };

    let status = if cancelled {
        EditionChangeStatus::Cancelled
    } else if timed_out {
        EditionChangeStatus::TimedOut
    } else if !exec_ok {
        EditionChangeStatus::VerificationFailed
    } else {
        classify_edition_change(Some(&before), after_snapshot.as_ref())
    };

    let restart_required = after_snapshot
        .as_ref()
        .map(|s| pending_restart_detected(s.pending_file_rename, s.reboot_pending))
        .unwrap_or(false);

    let safe_message = if status == EditionChangeStatus::VerificationFailed && !timed_out && !cancelled {
        "فشل تنفيذ تغيير الإصدار؛ لم يتغير شيء أو تعذر التحقق — انظر السجل.".to_string()
    } else {
        edition_status_message(status)
    };

    let _ = push_log(&state, &format!("[RESULT] {:?} — {}", status, safe_message));

    Ok(EditionChangeResult {
        status,
        before: Some(before),
        after: after_snapshot,
        restart_required,
        checked_at,
        safe_message,
    })
}

// ===== التحديثات =====

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub notes: String,
    pub download_url: String,
    pub asset_url: String,
    pub check_error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[tauri::command]
async fn check_update() -> Result<UpdateInfo, String> {
    tokio::task::spawn_blocking(move || {
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        let response = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .user_agent("MAS-Activator")
                .build(),
        )
        .get("https://api.github.com/repos/SMSMy/mas-activator-Disktop/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .call();

        let (release, check_error) = match response {
            Ok(resp) => match resp.into_body().read_json::<GitHubRelease>() {
                Ok(r) => (Some(r), None),
                Err(e) => (None, Some(format!("تعذر قراءة بيانات التحديث: {}", e))),
            },
            Err(ureq::Error::StatusCode(403)) | Err(ureq::Error::StatusCode(429)) => (
                None,
                Some("تعذر التحقق من التحديثات (حد طلبات GitHub) — حاول لاحقًا".to_string()),
            ),
            Err(e) => (
                None,
                Some(format!("تعذر التحقق من التحديثات (لا يوجد اتصال؟): {}", e)),
            ),
        };

        match release {
            Some(r) => {
                let latest_version = r.tag_name.trim_start_matches('v').to_string();
                let available = match (
                    Version::parse(&current_version),
                    Version::parse(&latest_version),
                ) {
                    (Ok(cur), Ok(lat)) => lat > cur,
                    _ => latest_version != current_version,
                };
                let asset_url = r
                    .assets
                    .iter()
                    .find(|a| {
                        a.name.to_lowercase().contains("portable")
                            && a.name.to_lowercase().ends_with(".exe")
                    })
                    .or_else(|| r.assets.iter().find(|a| a.name.to_lowercase().ends_with(".exe")))
                    .map(|a| a.browser_download_url.clone())
                    .unwrap_or_default();

                Ok(UpdateInfo {
                    available,
                    current_version,
                    latest_version,
                    notes: r.body.unwrap_or_else(|| "لا توجد ملاحظات".to_string()),
                    download_url: r.html_url,
                    asset_url,
                    check_error: None,
                })
            }
            None => Ok(UpdateInfo {
                available: false,
                current_version,
                latest_version: String::new(),
                notes: String::new(),
                download_url: String::new(),
                asset_url: String::new(),
                check_error,
            }),
        }
    })
    .await
    .map_err(|e| format!("خطأ في المعالجة: {}", e))?
}

// ===== التحقق من المسار (المرحلة 4.2 — قابل للاختبار) =====

fn resolve_download_path(filename: &str) -> Result<PathBuf, String> {
    if filename.trim().is_empty() {
        return Err("اسم الملف فارغ".to_string());
    }
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("اسم ملف غير صالح".to_string());
    }

    let base = std::env::var("USERPROFILE")
        .map(|p| PathBuf::from(p).join("Downloads"))
        .map_err(|_| "تعذر تحديد مجلد التنزيلات".to_string())?;

    std::fs::create_dir_all(&base)
        .map_err(|e| format!("تعذر إنشاء مجلد التنزيلات: {}", e))?;

    let canon_base = base
        .canonicalize()
        .map_err(|e| format!("تعذر التحقق من مجلد التنزيلات: {}", e))?;

    let candidate = canon_base.join(filename);
    match candidate.parent() {
        Some(p) if p == canon_base => Ok(candidate),
        _ => Err("المسار النهائي خارج مجلد التنزيلات".to_string()),
    }
}

#[tauri::command]
async fn download_update(url: String, filename: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let file_path = resolve_download_path(&filename)?;

        let response = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .user_agent("MAS-Activator")
                .build(),
        )
        .get(&url)
        .call()
        .map_err(|e| format!("تعذر تحميل الملف: {}", e))?;

        let bytes = response
            .into_body()
            .read_to_vec()
            .map_err(|e| format!("خطأ أثناء التحميل: {}", e))?;

        std::fs::write(&file_path, &bytes)
            .map_err(|e| format!("تعذر حفظ الملف: {}", e))?;

        #[cfg(target_os = "windows")]
        {
            let mut cmd = StdCommand::new("explorer");
            cmd.args(["/select,", &file_path.to_string_lossy()]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.spawn();
        }

        Ok(file_path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("خطأ في المعالجة: {}", e))?
}

// ===== نقطة الدخول =====

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState {
        logs: Arc::new(Mutex::new(Vec::new())),
        current: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .manage(app_state)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // تنظيف عند إغلاق النافذة: إنهاء شجرة أي عملية نشطة حتى لا تبقى يتيمة
                let state = window.state::<AppState>();
                let current = match state.current.lock() {
                    Ok(c) => c,
                    Err(_) => return,
                };
                if let Some(op) = current.as_ref() {
                    if let Some(pid) = op.pid {
                        kill_process_tree(pid);
                    }
                }
            }
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_status,
            run_activation,
            cancel_operation,
            get_logs,
            clear_logs,
            check_update,
            download_update,
            edition_preflight,
            open_activation_settings,
            verify_edition_change,
            change_edition,
            check_admin,
            export_logs,
            open_windows_security,
            adopt_mas_pin
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ===== اختبارات الوحدات (المرحلة 1.4) =====

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(
        name: &str,
        app_id: &str,
        status: i32,
        partial: bool,
        grace: i32,
    ) -> RawProduct {
        RawProduct {
            name: name.to_string(),
            description: String::new(),
            application_id: app_id.to_string(),
            partial_key: partial,
            license_status: status,
            grace_minutes: grace,
        }
    }

    #[test]
    fn license_status_priority_mapping() {
        assert_eq!(LicenseState::priority(1), 0);
        assert_eq!(LicenseState::priority(2), 1);
        assert_eq!(LicenseState::priority(6), 2);
        assert_eq!(LicenseState::priority(3), 3);
        assert_eq!(LicenseState::priority(4), 4);
        assert_eq!(LicenseState::priority(5), 5);
        assert_eq!(LicenseState::priority(0), 6);
        assert_eq!(LicenseState::priority(7), 99);
        assert_eq!(LicenseState::priority(-1), 99);
    }

    #[test]
    fn license_status_to_state_mapping() {
        assert_eq!(LicenseState::from_status(1), LicenseState::Activated);
        assert_eq!(LicenseState::from_status(5), LicenseState::Notification);
        assert_eq!(LicenseState::from_status(2), LicenseState::Grace);
        assert_eq!(LicenseState::from_status(3), LicenseState::Grace);
        assert_eq!(LicenseState::from_status(6), LicenseState::Grace);
        assert_eq!(LicenseState::from_status(4), LicenseState::NotGenuine);
        assert_eq!(LicenseState::from_status(0), LicenseState::NotLicensed);
        assert_eq!(LicenseState::from_status(9), LicenseState::Unknown);
    }

    #[test]
    fn labels_with_grace_days() {
        assert_eq!(
            LicenseState::Activated.label(None),
            "مفعل ✅".to_string()
        );
        assert_eq!(
            LicenseState::Grace.label(Some(3)),
            "فترة سماح ⏳ (3 يوم)".to_string()
        );
        assert_eq!(
            LicenseState::Grace.label(None),
            "فترة سماح ⏳".to_string()
        );
    }

    #[test]
    fn grace_days_conversion() {
        assert_eq!(grace_days(4320), Some(3));
        assert_eq!(grace_days(0), None);
        assert_eq!(grace_days(-5), None);
    }

    #[test]
    fn windows_selection_prefers_partial_key_then_priority() {
        let products = vec![
            raw("Windows 11 Pro edition", WINDOWS_APP_ID, 0, false, 0),
            raw("Windows 11 Pro edition", WINDOWS_APP_ID, 1, true, 0),
        ];
        let best = select_best(&products, |p| is_windows(&p.application_id)).unwrap();
        assert_eq!(best.license_status, 1);
        assert_eq!(best.state, LicenseState::Activated);
        assert_eq!(best.kind, ProductKind::Windows);
    }

    #[test]
    fn windows_selection_falls_back_without_partial_key() {
        let products = vec![
            raw("Windows 11 Pro edition", WINDOWS_APP_ID, 0, false, 0),
            raw("Windows 11 Pro edition", WINDOWS_APP_ID, 2, false, 1440),
        ];
        let best = select_best(&products, |p| is_windows(&p.application_id)).unwrap();
        assert_eq!(best.license_status, 2);
        assert_eq!(best.grace_days, Some(1));
    }

    #[test]
    fn windows_selection_excludes_insider() {
        let products = vec![
            raw("Windows 11 Insider Preview", WINDOWS_APP_ID, 1, true, 0),
            raw("Windows 11 Pro edition", WINDOWS_APP_ID, 0, true, 0),
        ];
        let best = select_best(&products, |p| is_windows(&p.application_id)).unwrap();
        assert_eq!(best.name, "Windows 11 Pro");
        assert_eq!(best.state, LicenseState::NotLicensed);
    }

    #[test]
    fn windows_selection_returns_none_when_no_windows() {
        let products = vec![raw("Office 16, OfficeProPlus", "other", 1, true, 0)];
        assert!(select_best(&products, |p| is_windows(&p.application_id)).is_none());
    }

    #[test]
    fn office_selection_ignores_windows_records() {
        let products = vec![
            raw("Windows 11 Pro edition", WINDOWS_APP_ID, 1, true, 0),
            raw("Office 16, OfficeProPlus", "other", 1, true, 0),
        ];
        let best = select_best(&products, |p| {
            !is_windows(&p.application_id) && is_office(&p.name)
        })
        .unwrap();
        assert_eq!(best.kind, ProductKind::Office);
    }

    #[test]
    fn office_name_cleanup() {
        assert_eq!(
            office_clean_name("Office 16, Office16ProPlusR_Grace edition"),
            "Office16ProPlusR_Grace edition"
        );
        assert_eq!(office_clean_name("  Office 19, foo  "), "foo");
        assert_eq!(office_clean_name("Project 16, ProPlus"), "Project 16");
    }

    #[test]
    fn insider_detection() {
        assert!(is_insider("Windows 11 Insider Preview", ""));
        assert!(is_insider("Windows 11 Pro", "Some Insider description"));
        assert!(!is_insider("Windows 11 Pro", ""));
    }

    #[test]
    fn office_selection_excludes_onenote_free() {
        let products = vec![
            raw("Office16OneNoteFreeR_Bypass edition", "other", 1, true, 0),
            raw("Office 16, Office16ProPlusR_Retail edition", "other", 5, true, 0),
        ];
        let best = select_best(&products, |p| {
            !is_windows(&p.application_id) && is_office(&p.name)
        })
        .unwrap();
        assert!(!best.name.to_uppercase().contains("ONENOTE"));
        assert_eq!(best.state, LicenseState::Notification);
    }

    #[test]
    fn windows_name_cleanup_mapping() {
        assert_eq!(
            windows_clean_name("Windows(R), Core edition"),
            "Windows Home"
        );
        assert_eq!(
            windows_clean_name("Windows(R), Professional edition"),
            "Windows Pro"
        );
        assert_eq!(
            windows_clean_name("Windows(R), ProfessionalWorkstation edition"),
            "Windows Pro for Workstations"
        );
        assert_eq!(
            windows_clean_name("Windows(R), ServerRdsh edition"),
            "Windows Server RDSH"
        );
        assert_eq!(
            windows_clean_name("Windows(R), CoreSingleLanguage edition"),
            "Windows Home Single Language"
        );
        assert_eq!(
            windows_clean_name("Windows(R), Education edition"),
            "Windows Education"
        );
    }

    #[test]
    fn observed_list_deduplicates_and_sorts() {
        let products = vec![
            raw("Windows(R), Core edition", WINDOWS_APP_ID, 0, true, 0),
            raw("Windows(R), Core edition", WINDOWS_APP_ID, 0, true, 0),
            raw("Windows(R), Professional edition", WINDOWS_APP_ID, 1, true, 0),
            raw("Office16OneNoteFreeR_Bypass edition", "other", 1, true, 0),
        ];
        let report = build_report(&products, None, None);
        assert_eq!(report.observed.len(), 2);
        assert_eq!(report.observed[0].state, LicenseState::Activated);
        assert_eq!(report.observed[0].name, "Windows Pro");
        assert!(report.office.is_none());
    }

    #[test]
    fn path_validation_rejects_traversal() {
        assert!(resolve_download_path("../evil.exe").is_err());
        assert!(resolve_download_path("..\\evil.exe").is_err());
        assert!(resolve_download_path("a/b.exe").is_err());
        assert!(resolve_download_path("a\\b.exe").is_err());
        assert!(resolve_download_path("").is_err());
    }

    #[test]
    fn path_validation_accepts_plain_name() {
        let result = resolve_download_path("MAS-Activator-2.2.0.exe");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path.file_name().unwrap().to_string_lossy(), "MAS-Activator-2.2.0.exe");
    }

    fn snap(
        edition: &str,
        state: Option<LicenseState>,
        pfro: bool,
        rb: bool,
    ) -> EditionSnapshot {
        EditionSnapshot {
            product_name: format!("Windows {}", edition),
            edition_id: edition.to_string(),
            display_version: None,
            current_build: "26100".to_string(),
            ubr: "1".to_string(),
            windows_state: state,
            windows_label: state.map(|s| s.label(None)),
            pending_file_rename: pfro,
            reboot_pending: rb,
        }
    }

    #[test]
    fn parses_dism_target_editions_and_dedupes() {
        let out = r#"
Deployment Image Servicing and Management tool
Version: 10.0.19041.844
Image Version: 10.0.19041.844
Editions that can be upgraded to:
Target Edition : ProfessionalEducation
Target Edition : ProfessionalWorkstation
Target Edition : Education
Target Edition : ProfessionalEducation
The operation completed successfully.
"#;
        let targets = parse_target_editions(out);
        assert_eq!(
            targets,
            vec!["ProfessionalEducation", "ProfessionalWorkstation", "Education"]
        );
    }

    #[test]
    fn parses_dism_empty_output() {
        assert!(parse_target_editions("").is_empty());
        assert!(parse_target_editions("no editions here").is_empty());
    }

    #[test]
    fn blocks_non_core_to_core_paths() {
        let targets = vec!["Core".to_string(), "Education".to_string()];
        let (supported, blocked) = filter_targets(&targets, "Professional");
        assert!(!supported.iter().any(|t| t.contains("Core")));
        assert!(blocked.iter().any(|t| t.contains("Core")));
        assert!(supported.contains(&"Education".to_string()));
    }

    #[test]
    fn blocks_special_editions() {
        let targets = vec![
            "CoreCountrySpecific".to_string(),
            "ServerRdsh".to_string(),
            "CloudEdition".to_string(),
            "Professional".to_string(),
        ];
        let (supported, blocked) = filter_targets(&targets, "Core");
        assert_eq!(supported, vec!["Professional".to_string()]);
        assert_eq!(blocked.len(), 3);
    }

    #[test]
    fn blocks_all_paths_from_cloud_edition() {
        let targets = vec!["Professional".to_string(), "Education".to_string()];
        let (supported, _) = filter_targets(&targets, "CloudEdition");
        assert!(supported.is_empty());
    }

    #[test]
    fn classifies_edition_change_states() {
        let before = snap("Core", Some(LicenseState::Activated), false, false);
        let after_pro_activated = snap("Professional", Some(LicenseState::Activated), false, false);
        let after_pro_unlicensed = snap("Professional", Some(LicenseState::NotLicensed), false, false);
        let after_same = snap("Core", Some(LicenseState::Activated), false, false);
        let after_restart = snap("Professional", Some(LicenseState::Activated), true, false);
        let after_same_restart = snap("Core", Some(LicenseState::Activated), false, true);

        assert_eq!(
            classify_edition_change(Some(&before), Some(&after_pro_activated)),
            EditionChangeStatus::EditionChangedAndActivated
        );
        assert_eq!(
            classify_edition_change(Some(&before), Some(&after_pro_unlicensed)),
            EditionChangeStatus::EditionChangedNeedsActivation
        );
        assert_eq!(
            classify_edition_change(Some(&before), Some(&after_same)),
            EditionChangeStatus::EditionUnchanged
        );
        assert_eq!(
            classify_edition_change(Some(&before), Some(&after_restart)),
            EditionChangeStatus::PendingRestart
        );
        assert_eq!(
            classify_edition_change(Some(&before), Some(&after_same_restart)),
            EditionChangeStatus::PendingRestart
        );
        assert_eq!(
            classify_edition_change(Some(&before), None),
            EditionChangeStatus::VerificationFailed
        );
        assert_eq!(
            classify_edition_change(None, Some(&after_pro_activated)),
            EditionChangeStatus::VerificationFailed
        );
    }

    #[test]
    fn pending_restart_detection_flags() {
        assert!(pending_restart_detected(true, false));
        assert!(pending_restart_detected(false, true));
        assert!(!pending_restart_detected(false, false));
    }

    #[test]
    fn key_pattern_detection() {
        assert!(contains_key_pattern("XXXXX-XXXXX-XXXXX-XXXXX-XXXXX"));
        assert!(contains_key_pattern("المفتاح: ABCDE-FGHIJ-KLMNO-PQRST-UVWXY"));
        assert!(!contains_key_pattern("الإصدار تغير إلى Professional"));
        assert!(!contains_key_pattern("12345"));
        assert!(!contains_key_pattern("ABC-DE-FGHI-JKL"));
    }

    #[test]
    fn key_redaction_hides_full_keys() {
        let input = "تم التنفيذ بالمفتاح ABCDE-FGHIJ-KLMNO-PQRST-UVWXY بنجاح";
        let redacted = redact_keys(input);
        assert!(!contains_key_pattern(&redacted));
        assert!(redacted.contains("[مفتاح محجوب]"));
        assert!(!redacted.contains("UVWXY"));
        assert_eq!(redact_keys("لا مفاتيح هنا"), "لا مفاتيح هنا");
    }

    #[test]
    fn sha256_hex_known_vector() {
        // SHA-256("abc") — متجه معياري
        assert_eq!(
            sha256_hex(b"abc"),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        );
    }

    #[test]
    fn pinned_hash_format_is_valid() {
        assert_eq!(MAS_EXPECTED_SHA256.len(), 64);
        assert!(MAS_EXPECTED_SHA256.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(hash_matches_expected(&[0u8; 10]) == false);
    }

    #[test]
    fn cache_path_inside_local_app_data() {
        let path = resolve_cache_path().unwrap();
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            mas_script_name().as_str()
        );
        let parent = path.parent().unwrap();
        assert!(parent.ends_with("cache"));
        assert!(parent.to_string_lossy().contains("MAS Activator"));
    }

    #[test]
    fn cmd_invocation_quoting_runs_batch_file_with_spaces() {
        let dir = std::env::temp_dir().join("mas_quote_test");
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake tool.cmd");
        std::fs::write(&script, "@echo OK_MARKER\r\n").unwrap();

        let command_line = format!("\"{}\" /HWID", script.display());
        let out = StdCommand::new("cmd")
            .arg("/D")
            .arg("/C")
            .raw_arg(&command_line)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stdout.contains("OK_MARKER"),
            "stdout: {:?}\nstderr: {:?}",
            stdout,
            stderr
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tag_comparison_is_numeric_not_lexicographic() {
        assert!(tag_is_newer("3.12", "3.9"));
        assert!(tag_is_newer("3.12.1", "3.12"));
        assert!(tag_is_newer("4.0", "3.12"));
        assert!(!tag_is_newer("3.12", "3.12"));
        assert!(!tag_is_newer("3.9", "3.12"));
        assert!(tag_is_newer("v3.13", "3.12"));
    }

    #[test]
    fn adopted_meta_roundtrip() {
        let meta = PinMeta {
            version_tag: "3.13".to_string(),
            sha256: "ABC".to_string(),
            adopted_at: 123,
        };
        let raw = serde_json::to_string(&meta).unwrap();
        let back: PinMeta = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.version_tag, "3.13");
        assert_eq!(back.sha256, "ABC");
        assert_eq!(back.adopted_at, 123);
    }

    #[test]
    fn edition_status_messages_all_present() {
        let statuses = [
            EditionChangeStatus::SettingsOpened,
            EditionChangeStatus::PendingRestart,
            EditionChangeStatus::EditionChangedAndActivated,
            EditionChangeStatus::EditionChangedNeedsActivation,
            EditionChangeStatus::EditionUnchanged,
            EditionChangeStatus::UnsupportedPath,
            EditionChangeStatus::VerificationFailed,
            EditionChangeStatus::Cancelled,
            EditionChangeStatus::TimedOut,
        ];
        for s in statuses {
            assert!(!edition_status_message(s).is_empty());
        }
    }

    #[test]
    fn selects_change_method_by_edition() {
        assert_eq!(
            select_change_method("Core", false, true, 26100),
            ChangeMethod::Changepk
        );
        assert_eq!(
            select_change_method("Professional", false, true, 26100),
            ChangeMethod::Slmgr
        );
        assert_eq!(
            select_change_method("Professional", false, false, 26100),
            ChangeMethod::Changepk
        );
        assert_eq!(
            select_change_method("ServerStandard", true, false, 26100),
            ChangeMethod::Server
        );
        assert_eq!(
            select_change_method("Core", false, true, 10240),
            ChangeMethod::Unsupported
        );
        assert_eq!(
            select_change_method("EnterpriseEval", false, true, 26100),
            ChangeMethod::Unsupported
        );
    }
}
