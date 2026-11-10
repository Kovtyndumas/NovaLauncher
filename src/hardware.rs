use serde_json::{json, Value};
use std::sync::OnceLock;
pub fn info() -> Value {
    static INFO: OnceLock<Value> = OnceLock::new();
    INFO.get_or_init(detect).clone()
}
fn output(command: &str, args: &[&str]) -> String {
    std::process::Command::new(command)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default()
}
fn detect() -> Value {
    let mut result = json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"cores":std::thread::available_parallelism().map(|n|n.get()).unwrap_or(1)});
    #[cfg(target_os = "macos")]
    {
        result["cpu"] = json!(output(
            "/usr/sbin/sysctl",
            &["-n", "machdep.cpu.brand_string"]
        ));
        let bytes = output("/usr/sbin/sysctl", &["-n", "hw.memsize"])
            .parse::<u64>()
            .unwrap_or(0);
        result["ramGiB"] = json!(bytes / (1024 * 1024 * 1024));
        let data = output(
            "/usr/sbin/system_profiler",
            &["SPDisplaysDataType", "-json"],
        );
        if let Ok(v) = serde_json::from_str::<Value>(&data) {
            result["gpu"] = v["SPDisplaysDataType"][0]["sppci_model"].clone();
        }
    }
    #[cfg(target_os = "linux")]
    {
        let mem = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
        let kb = mem
            .lines()
            .find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        result["ramGiB"] = json!((kb + 524288) / 1048576);
        let cpu = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        result["cpu"] = json!(cpu
            .lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, s)| s.trim())
            .unwrap_or(""));
    }
    #[cfg(target_os = "windows")]
    {
        let data=output("powershell",&["-NoProfile","-NonInteractive","-Command","$c=Get-CimInstance Win32_Processor | Select-Object -First 1; $m=Get-CimInstance Win32_ComputerSystem; $g=Get-CimInstance Win32_VideoController | Select-Object -First 1; @{cpu=$c.Name;ramGiB=[math]::Round($m.TotalPhysicalMemory/1GB);gpu=$g.Name} | ConvertTo-Json -Compress"]);
        if let Ok(v) = serde_json::from_str::<Value>(&data) {
            for k in ["cpu", "ramGiB", "gpu"] {
                result[k] = v[k].clone();
            }
        }
    }
    result
}
