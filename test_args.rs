fn main() {
    let args: Vec<String> = vec![
        "save-cube".to_string(),
        "project".to_string(),
        "测试内容".to_string(),
        "#tag1".to_string(),
        "#tag2".to_string(),
    ];
    
    println!("args len: {}", args.len());
    println!("args[2]: {}", args[2]);
    println!("args[3]: {}", args[3]);
    println!("args[4]: {}", args[4]);
    
    // 检查是否有 tags
    let has_tags = args.len() > 4 && args[4].starts_with('#');
    println!("has_tags: {}", has_tags);
}
