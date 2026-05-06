use semver::Version;
use serde::Deserialize;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tauri::State;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

const UTF8_PREFIX: &str = r#"chcp 65001 > $null 2>&1; [Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; "#;

#[derive(Clone)]
pub struct AppState {
    pub logs: Arc<Mutex<Vec<String>>>,
}

#[derive(Debug, Deserialize, Clone, serde::Serialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub notes: String,
    pub download_url: String,
    pub asset_url: String,
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

/// دالة مساعدة آمنة لإضافة سطر إلى سجل التطبيق بدون panic
fn push_log(state: &State<'_, AppState>, entry: &str) -> Result<(), String> {
    let mut logs = state
        .logs
        .lock()
        .map_err(|_| "تعذر الوصول إلى سجل التطبيق".to_string())?;
    logs.push(entry.to_string());
    Ok(())
}

#[tauri::command]
async fn activate_windows(state: State<'_, AppState>) -> Result<String, String> {
    let _ = push_log(&state, "[INFO] جاري تفعيل ويندوز (HWID)...");
    
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
try {
    $masScript = irm https://get.activated.win -UseBasicParsing
    $output = & ([ScriptBlock]::Create($masScript)) /HWID 2>&1 | Out-String
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        Write-Output "ACTIVATION_FAILED|$output"
    } else {
        Write-Output "ACTIVATION_OK|$output"
    }
} catch {
    try {
        $masScript = curl.exe -s --doh-url https://1.1.1.1/dns-query https://get.activated.win | Out-String
        $output = & ([ScriptBlock]::Create($masScript)) /HWID 2>&1 | Out-String
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            Write-Output "ACTIVATION_FAILED|$output"
        } else {
            Write-Output "ACTIVATION_OK|$output"
        }
    } catch {
        Write-Output "ACTIVATION_ERROR|خطأ: $($_.Exception.Message)"
    }
}
"#;

    let result = tokio::task::spawn_blocking(move || execute_activation(script))
        .await
        .map_err(|e| format!("خطأ في المعالجة: {}", e))?;

    match &result {
        Ok(output) => { let _ = push_log(&state, &format!("[SUCCESS] {}", output)); }
        Err(e) => { let _ = push_log(&state, &format!("[ERROR] {}", e)); }
    }
    result
}

#[tauri::command]
async fn activate_office(state: State<'_, AppState>) -> Result<String, String> {
    let _ = push_log(&state, "[INFO] جاري تفعيل أوفيس (Ohook)...");
    
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
try {
    $masScript = irm https://get.activated.win -UseBasicParsing
    $output = & ([ScriptBlock]::Create($masScript)) /Ohook 2>&1 | Out-String
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        Write-Output "ACTIVATION_FAILED|$output"
    } else {
        Write-Output "ACTIVATION_OK|$output"
    }
} catch {
    try {
        $masScript = curl.exe -s --doh-url https://1.1.1.1/dns-query https://get.activated.win | Out-String
        $output = & ([ScriptBlock]::Create($masScript)) /Ohook 2>&1 | Out-String
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            Write-Output "ACTIVATION_FAILED|$output"
        } else {
            Write-Output "ACTIVATION_OK|$output"
        }
    } catch {
        Write-Output "ACTIVATION_ERROR|خطأ: $($_.Exception.Message)"
    }
}
"#;

    let result = tokio::task::spawn_blocking(move || execute_activation(script))
        .await
        .map_err(|e| format!("خطأ في المعالجة: {}", e))?;

    match &result {
        Ok(output) => { let _ = push_log(&state, &format!("[SUCCESS] {}", output)); }
        Err(e) => { let _ = push_log(&state, &format!("[ERROR] {}", e)); }
    }
    result
}

#[tauri::command]
async fn activate_all(state: State<'_, AppState>) -> Result<String, String> {
    let _ = push_log(&state, "[INFO] جاري تفعيل الكل (ويندوز + أوفيس)...");
    
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
try {
    $masScript = irm https://get.activated.win -UseBasicParsing
    $output = & ([ScriptBlock]::Create($masScript)) /HWID /Ohook 2>&1 | Out-String
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        Write-Output "ACTIVATION_FAILED|$output"
    } else {
        Write-Output "ACTIVATION_OK|$output"
    }
} catch {
    try {
        $masScript = curl.exe -s --doh-url https://1.1.1.1/dns-query https://get.activated.win | Out-String
        $output = & ([ScriptBlock]::Create($masScript)) /HWID /Ohook 2>&1 | Out-String
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            Write-Output "ACTIVATION_FAILED|$output"
        } else {
            Write-Output "ACTIVATION_OK|$output"
        }
    } catch {
        Write-Output "ACTIVATION_ERROR|خطأ: $($_.Exception.Message)"
    }
}
"#;

    let result = tokio::task::spawn_blocking(move || execute_activation(script))
        .await
        .map_err(|e| format!("خطأ في المعالجة: {}", e))?;

    match &result {
        Ok(output) => { let _ = push_log(&state, &format!("[SUCCESS] {}", output)); }
        Err(e) => { let _ = push_log(&state, &format!("[ERROR] {}", e)); }
    }
    result
}

#[tauri::command]
async fn activate_tsforge(state: State<'_, AppState>) -> Result<String, String> {
    let _ = push_log(&state, "[INFO] جاري تفعيل الكل عبر TSforge...");
    
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
try {
    $masScript = irm https://get.activated.win -UseBasicParsing
    $output = & ([ScriptBlock]::Create($masScript)) /Z-WindowsESUOffice 2>&1 | Out-String
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        Write-Output "ACTIVATION_FAILED|$output"
    } else {
        Write-Output "ACTIVATION_OK|$output"
    }
} catch {
    try {
        $masScript = curl.exe -s --doh-url https://1.1.1.1/dns-query https://get.activated.win | Out-String
        $output = & ([ScriptBlock]::Create($masScript)) /Z-WindowsESUOffice 2>&1 | Out-String
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            Write-Output "ACTIVATION_FAILED|$output"
        } else {
            Write-Output "ACTIVATION_OK|$output"
        }
    } catch {
        Write-Output "ACTIVATION_ERROR|خطأ: $($_.Exception.Message)"
    }
}
"#;

    let result = tokio::task::spawn_blocking(move || execute_activation(script))
        .await
        .map_err(|e| format!("خطأ في المعالجة: {}", e))?;

    match &result {
        Ok(output) => { let _ = push_log(&state, &format!("[SUCCESS] {}", output)); }
        Err(e) => { let _ = push_log(&state, &format!("[ERROR] {}", e)); }
    }
    result
}

#[tauri::command]
async fn activate_kms(state: State<'_, AppState>) -> Result<String, String> {
    let _ = push_log(&state, "[INFO] جاري تفعيل الكل عبر Online KMS...");
    
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
try {
    $masScript = irm https://get.activated.win -UseBasicParsing
    $output = & ([ScriptBlock]::Create($masScript)) /K-WindowsOffice 2>&1 | Out-String
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        Write-Output "ACTIVATION_FAILED|$output"
    } else {
        Write-Output "ACTIVATION_OK|$output"
    }
} catch {
    try {
        $masScript = curl.exe -s --doh-url https://1.1.1.1/dns-query https://get.activated.win | Out-String
        $output = & ([ScriptBlock]::Create($masScript)) /K-WindowsOffice 2>&1 | Out-String
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            Write-Output "ACTIVATION_FAILED|$output"
        } else {
            Write-Output "ACTIVATION_OK|$output"
        }
    } catch {
        Write-Output "ACTIVATION_ERROR|خطأ: $($_.Exception.Message)"
    }
}
"#;

    let result = tokio::task::spawn_blocking(move || execute_activation(script))
        .await
        .map_err(|e| format!("خطأ في المعالجة: {}", e))?;

    match &result {
        Ok(output) => { let _ = push_log(&state, &format!("[SUCCESS] {}", output)); }
        Err(e) => { let _ = push_log(&state, &format!("[ERROR] {}", e)); }
    }
    result
}

#[tauri::command]
async fn check_status(state: State<'_, AppState>) -> Result<String, String> {
    let _ = push_log(&state, "[INFO] جاري فحص حالة التفعيل...");
    
    let script = r#"
$ProgressPreference = 'SilentlyContinue'
$ErrorActionPreference = 'SilentlyContinue'
try {
    # ===== Windows Check =====
    $win_status = 'غير معروف'
    $win_output = & cscript //Nologo "$env:windir\system32\slmgr.vbs" /xpr 2>&1
    $win_line = ($win_output | Out-String).Trim() -replace "`r`n", " " -replace "`n", " "
    
    if ($win_line -match 'permanently activated' -or $win_line -match 'permanent') {
        $win_status = 'مفعل بشكل دائم ✅'
    }
    elseif ($win_line -match 'will expire\s+(\d.+)') {
        $win_status = 'مفعل مؤقتاً ⏳'
    }
    elseif ($win_line -match 'notification') {
        $win_status = 'غير مفعل ❌'
    }
    elseif ($win_line -match 'grace') {
        $win_status = 'فترة سماح ⏳'
    }
    elseif ($win_line -match 'not activated' -or $win_line -match 'not found') {
        $win_status = 'غير مفعل ❌'
    }
    elseif ($win_line -match 'activated') {
        $win_status = 'مفعل ✅'
    }
    else {
        $win_status = $win_line
    }

    # ===== Office Check =====
    $office_status = 'غير مثبت'
    $office_name = ''

    $office_products = Get-CimInstance -ClassName SoftwareLicensingProduct -Filter "Name like '%Office%' AND PartialProductKey IS NOT NULL" 2>$null
    
    if (-not $office_products) {
        $office_products = Get-CimInstance -ClassName SoftwareLicensingProduct -Filter "Name like '%Office%' AND LicenseStatus > 0" 2>$null
    }

    if ($office_products) {
        # ترتيب أولوية صحيح:
        # 1 (مفعل) أولاً، ثم 2،6 (فترة سماح)، ثم الباقي
        # الترقيم الرقمي وحده مُضلِّل: 5 > 1 رقمياً لكن 1 هو الأفضل ترخيصاً
        $priority = @{1=0; 2=1; 6=2; 3=3; 4=4; 5=5; 0=6}
        $product = $office_products | Sort-Object -Property {
            $p = $priority[$_.LicenseStatus]
            if ($null -eq $p) { 99 } else { $p }
        } | Select-Object -First 1
        $lic = $product.LicenseStatus
        $office_name = $product.Name -replace 'Office.*?,\s*', '' -replace ',.*', ''
        if (-not $office_name) { $office_name = $product.Name }

        switch ($lic) {
            1 { $office_status = 'مفعل ✅' }
            0 { $office_status = 'غير مرخص ❌' }
            2 { $office_status = 'فترة سماح - OOBE ⏳' }
            3 { $office_status = 'فترة سماح - OOT ⏳' }
            4 { $office_status = 'نسخة غير أصلية ❌' }
            5 { $office_status = 'إشعار ترخيص ⚠️' }
            6 { $office_status = 'فترة سماح ممتدة ⏳' }
            default { $office_status = "حالة غير معروفة (كود: $lic)" }
        }
    } else {
        $officeInstalled = Test-Path "$env:ProgramFiles\Microsoft Office" -ErrorAction SilentlyContinue
        if (-not $officeInstalled) {
            $officeInstalled = Test-Path "${env:ProgramFiles(x86)}\Microsoft Office" -ErrorAction SilentlyContinue
        }
        if ($officeInstalled) {
            $office_status = 'مثبت لكن بدون مفتاح ❌'
        } else {
            $office_status = 'غير مثبت'
        }
    }

    $result = @{
        windows = $win_status
        office = $office_status
        office_name = $office_name
    } | ConvertTo-Json -Compress
    
    Write-Output $result
} catch {
    $err = $_.Exception.Message
    if (-not $err) { $err = 'خطأ غير معروف' }
    Write-Output ('{"windows":"خطأ في الفحص","office":"خطأ في الفحص","office_name":"","error":"' + ($err -replace '"','\"') + '"}')
}
"#;

    let result = tokio::task::spawn_blocking(move || execute_powershell(script))
        .await
        .map_err(|e| format!("خطأ في المعالجة: {}", e))?;

    match &result {
        Ok(output) => { let _ = push_log(&state, &format!("[SUCCESS] {}", output)); }
        Err(e) => { let _ = push_log(&state, &format!("[ERROR] {}", e)); }
    }
    result
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
        .call()
        .map_err(|e| format!("تعذر الاتصال بـ GitHub: {}", e))?;

        let release: GitHubRelease = response
            .into_body()
            .read_json()
            .map_err(|e| format!("تعذر قراءة بيانات التحديث: {}", e))?;

        let latest_version = release
            .tag_name
            .trim_start_matches('v')
            .to_string();

        // استخدام semver للمقارنة الصحيحة بين الإصدارات
        let available = match (
            Version::parse(&current_version),
            Version::parse(&latest_version),
        ) {
            (Ok(cur), Ok(lat)) => lat > cur,
            _ => latest_version != current_version, // fallback للمقارنة النصية
        };
        let notes = release.body.unwrap_or_else(|| "لا توجد ملاحظات".to_string());
        let download_url = release.html_url;

        // Find the .exe asset (portable or setup)
        let asset_url = release.assets.iter()
            .find(|a| a.name.to_lowercase().ends_with(".exe"))
            .map(|a| a.browser_download_url.clone())
            .unwrap_or_default();

        Ok(UpdateInfo {
            available,
            current_version,
            latest_version,
            notes,
            download_url,
            asset_url,
        })
    })
    .await
    .map_err(|e| format!("خطأ في المعالجة: {}", e))?
}

#[tauri::command]
async fn download_update(url: String, filename: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        // Determine download path (user's Downloads folder)
        let downloads_dir = std::env::var("USERPROFILE")
            .map(|p| std::path::PathBuf::from(p).join("Downloads"))
            .unwrap_or_else(|_| std::env::temp_dir());

        let file_path = downloads_dir.join(&filename);

        let response = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .user_agent("MAS-Activator")
                .build(),
        )
        .get(&url)
        .call()
        .map_err(|e| format!("تعذر تحميل الملف: {}", e))?;

        let bytes = response.into_body()
            .read_to_vec()
            .map_err(|e| format!("خطأ أثناء التحميل: {}", e))?;

        std::fs::write(&file_path, &bytes)
            .map_err(|e| format!("تعذر حفظ الملف: {}", e))?;

        // Open the Downloads folder with the file selected
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("explorer")
                .args(&["/select,", &file_path.to_string_lossy()])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }

        Ok(file_path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| format!("خطأ في المعالجة: {}", e))?
}

fn execute_powershell(script: &str) -> Result<String, String> {
    let full_script = format!("{}{}", UTF8_PREFIX, script);

    let mut cmd = Command::new("powershell");
    cmd.args(&["-NoProfile", "-Command", &full_script]);

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("خطأ في تنفيذ الأمر: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.status.success() {
        Ok(stdout.trim().to_string())
    } else {
        Err(if stderr.is_empty() { stdout } else { stderr }.trim().to_string())
    }
}

fn execute_activation(script: &str) -> Result<String, String> {
    let full_script = format!("{}{}", UTF8_PREFIX, script);

    let mut cmd = Command::new("powershell");
    cmd.args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &full_script]);

    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("خطأ في تنفيذ الأمر: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let result_text = stdout.trim().to_string();

    // Parse structured output from the PowerShell script
    if result_text.starts_with("ACTIVATION_OK|") {
        let details = result_text.trim_start_matches("ACTIVATION_OK|").trim();
        if details.is_empty() {
            Ok("تم التفعيل بنجاح".to_string())
        } else {
            Ok(format!("تم التفعيل بنجاح\n{}", details))
        }
    } else if result_text.starts_with("ACTIVATION_FAILED|") {
        let details = result_text.trim_start_matches("ACTIVATION_FAILED|").trim();
        Err(format!("فشل التفعيل\n{}", details))
    } else if result_text.starts_with("ACTIVATION_ERROR|") {
        let details = result_text.trim_start_matches("ACTIVATION_ERROR|").trim();
        Err(format!("خطأ في التفعيل: {}", details))
    } else if !output.status.success() {
        let error_info = if stderr.is_empty() { &stdout } else { &stderr };
        Err(format!("فشل التفعيل: {}", error_info.trim()))
    } else {
        // No structured prefix but process succeeded - return raw output
        Ok(if result_text.is_empty() {
            "تم التنفيذ".to_string()
        } else {
            result_text
        })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState {
        logs: Arc::new(Mutex::new(Vec::new())),
    };

    tauri::Builder::default()
        .manage(app_state)
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
            activate_windows,
            activate_office,
            activate_all,
            activate_tsforge,
            activate_kms,
            check_status,
            get_logs,
            clear_logs,
            check_update,
            download_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
