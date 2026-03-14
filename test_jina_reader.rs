// Test script for Jina Reader integration
use std::process::Command;

fn main() {
    println!("Testing Jina Reader integration...");
    
    // Test 1: Default URL (example.com)
    println!("
=== Test 1: Default URL ===");
    let output = Command::new("powershell")
        .args(&["-Command", "Set-Location 'C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\jina_reader'; .	argetelease\jina_reader.exe"])
        .output()
        .expect("Failed to execute jina_reader");
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("✅ Test 1 passed - Default URL works");
        // Print first 200 characters of content
        if let Some(content_start) = stdout.find(""content":") {
            let end_pos = (content_start + 200).min(stdout.len());
            println!("Content preview: {}", &stdout[content_start..end_pos]);
        }
    } else {
        println!("❌ Test 1 failed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Error: {}", stderr);
    }
    
    // Test 2: Specific URL (rust-lang.org)
    println!("
=== Test 2: Specific URL ===");
    let test_input = r#"{"url":"https://doc.rust-lang.org","timeout_seconds":15}"#;
    let output = Command::new("powershell")
        .args(&["-Command", &format!("Set-Location 'C:\Users\Administrator\AppData\Roaming\AionUi\aionui\gemini-temp-1772957577810\skills\jina_reader'; .	argetelease\jina_reader.exe '{}'", test_input)])
        .output()
        .expect("Failed to execute jina_reader with specific URL");
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains(""status":"success"") {
            println!("✅ Test 2 passed - Specific URL works");
        } else {
            println!("❌ Test 2 failed - Unexpected response");
            println!("Response: {}", &stdout[..std::cmp::min(500, stdout.len())]);
        }
    } else {
        println!("❌ Test 2 failed - Execution error");
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Error: {}", stderr);
    }
    
    println!("
=== Jina Reader Integration Test Complete ===");
}