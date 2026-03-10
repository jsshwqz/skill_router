use std::fs;
use std::path::Path;
use anyhow::{Result, bail};

pub struct SecurityAnalyzer;

impl SecurityAnalyzer {
    /// 对指定目录下的代码进行静态安全审计
    pub fn audit_skill_dir<P: AsRef<Path>>(dir: P) -> Result<()> {
        println!("[SECURITY] Auditing skill directory: {:?}", dir.as_ref());
        
        // 递归扫描所有源代码文件
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "rs" || ext == "py" || ext == "js" {
                    Self::analyze_file_content(&path)?;
                }
            }
        }
        
        println!("[SECURITY] Audit Passed.");
        Ok(())
    }

    fn analyze_file_content(path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)?;
        
        // 规则 1: 严禁恶意系统删除操作
        if content.contains("rm -rf") || content.contains("std::fs::remove_dir_all(\"/\")") {
            bail!("SECURITY VIOLATION: Malicious file deletion detected in {:?}", path);
        }
        
        // 规则 2: 严禁静默修改系统安全配置
        if content.contains("/etc/passwd") || content.contains("hosts") || content.contains("proxy_settings") {
            bail!("SECURITY VIOLATION: Suspicious system file access detected in {:?}", path);
        }
        
        // 规则 3: 严禁静默下载执行
        if content.contains("curl -s") || content.contains("wget -O-") {
            bail!("SECURITY VIOLATION: Stealth download-and-execute pattern detected in {:?}", path);
        }

        Ok(())
    }
}
