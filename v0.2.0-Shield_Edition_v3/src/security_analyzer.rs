use std::fs;
use std::path::Path;
use anyhow::{Result, bail};

pub struct SecurityAnalyzer;

impl SecurityAnalyzer {
    pub fn audit_skill_dir<P: AsRef<Path>>(dir: P) -> Result<()> {
        println!("[SECURITY] Auditing skill directory: {:?}", dir.as_ref());
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "rs" || ext == "py" || ext == "js" {
                    Self::analyze_file_content(&path)?;
                }
            } else if path.is_dir() {
                Self::audit_skill_dir(&path)?;
            }
        }
        
        println!("[SECURITY] Audit Passed.");
        Ok(())
    }

    fn analyze_file_content(path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)?;
        
        let dangerous_patterns = [
            ("rm -rf /", "Dangerous rm -rf command"),
            ("rm -rf *", "Dangerous rm -rf wildcard"),
            ("/etc/passwd", "System password file access"),
            ("/etc/shadow", "Shadow file access"),
            ("format c:", "Windows format command"),
            ("drop table", "SQL drop table"),
            ("eval(", "eval() call"),
            ("exec(", "exec() call"),
            ("__import__", "Python dynamic import"),
            ("os.system(", "OS system call"),
            ("subprocess", "Subprocess call"),
        ];
        
        let suspicious_patterns = [
            ("base64.b64decode", "Base64 decoding"),
            ("pickle.loads", "Pickle deserialization"),
            ("password =", "Hardcoded password"),
            ("api_key =", "Hardcoded API key"),
            ("secret =", "Hardcoded secret"),
        ];
        
        for (pattern, desc) in dangerous_patterns.iter() {
            if content.contains(pattern) {
                bail!("SECURITY VIOLATION: {} detected in {:?}", desc, path);
            }
        }
        
        for (pattern, desc) in suspicious_patterns.iter() {
            if content.contains(pattern) {
                println!("[SECURITY WARNING] {} in {:?}", desc, path);
            }
        }

        Ok(())
    }
}
