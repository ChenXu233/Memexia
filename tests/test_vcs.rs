//! VCS 模块集成测试

use memexia::vcs::{Vcs, CommitInfo};
use memexia::storage::Storage;

#[test]
fn test_commit_info_short() {
    let info = CommitInfo {
        oid: "abc123456789".to_string(),
        message: "Test commit".to_string(),
        graph_hash: Some("hash123".to_string()),
        author: "Test <test@example.com>".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
    };

    let short = info.to_short();
    assert_eq!(short, "abc1234 - Test commit");
}

#[test]
fn test_commit_info_short_long_oid() {
    let info = CommitInfo {
        oid: "1234567890abcdef".to_string(),
        message: "Another commit".to_string(),
        graph_hash: None,
        author: "User <user@test.com>".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
    };

    let short = info.to_short();
    assert_eq!(short, "1234567 - Another commit");
}

#[test]
fn test_vcs_init_and_open() {
    use tempfile::TempDir;
    use std::fs;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    // 初始化
    let vcs = Vcs::init(path).unwrap();
    assert!(path.join(".git").exists());

    // 打开已存在的
    let vcs2 = Vcs::open(path).unwrap();
    assert!(true); // 成功打开
}

#[test]
fn test_vcs_init_multiple_times() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    // 多次初始化应该成功（覆盖已存在的）
    let vcs1 = Vcs::init(path).unwrap();
    let vcs2 = Vcs::init(path).unwrap();
    assert!(true); // 两次初始化都成功
}

#[test]
fn test_vcs_commit_info() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);

    let mut vcs = Vcs::init(path).unwrap();

    // 创建提交获取 CommitInfo
    let storage = Storage::init(path).unwrap();

    // 创建一个测试文件
    let test_file = path.join("test.txt");
    std::fs::write(&test_file, "test content").unwrap();

    // 提交
    let oid = vcs.commit("Test commit", &[test_file.clone()], &storage).unwrap();
    assert!(!oid.is_empty());

    // 获取日志
    let logs = vcs.log(10).unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].message, "Test commit");
    assert_eq!(logs[0].oid, oid);
}

#[test]
fn test_vcs_log_limit() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);

    let mut vcs = Vcs::init(path).unwrap();
    let storage = Storage::init(path).unwrap();

    // 创建多个提交
    for i in 1..=5 {
        let test_file = path.join(format!("test{}.txt", i));
        std::fs::write(&test_file, format!("content {}", i)).unwrap();
        vcs.commit(&format!("Commit {}", i), &[test_file.clone()], &storage).unwrap();
    }

    // 限制获取数量
    let logs = vcs.log(3).unwrap();
    assert_eq!(logs.len(), 3);
}

#[test]
fn test_vcs_head_info() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);

    let mut vcs = Vcs::init(path).unwrap();
    let storage = Storage::init(path).unwrap();

    // 初始没有提交
    let head = vcs.head_info().unwrap();
    assert!(head.is_none());

    // 创建提交
    let test_file = path.join("test.txt");
    std::fs::write(&test_file, "content").unwrap();
    vcs.commit("First commit", &[test_file.clone()], &storage).unwrap();

    // 应该有 HEAD 信息
    let head = vcs.head_info().unwrap();
    assert!(head.is_some());
    assert_eq!(head.unwrap().message, "First commit");
}

#[test]
fn test_vcs_amend() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);

    let mut vcs = Vcs::init(path).unwrap();
    let storage = Storage::init(path).unwrap();

    // 创建初始提交
    let test_file = path.join("test.txt");
    std::fs::write(&test_file, "content").unwrap();
    let _oid1 = vcs.commit("Original message", &[test_file.clone()], &storage).unwrap();

    // 修改提交
    std::fs::write(&test_file, "updated content").unwrap();
    vcs.amend("Amended message", &storage).unwrap();

    // 检查日志
    let logs = vcs.log(1).unwrap();
    assert_eq!(logs[0].message, "Amended message");
}

/// 配置 Git 用户信息（Windows 需要）
fn configure_git_user(path: &std::path::Path) {
    let git_dir = path.join(".git");
    let config_path = git_dir.join("config");

    // 检查是否已有用户配置
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if content.contains("user.name") {
            return;
        }
    }

    // 添加用户配置
    let config_content = format!(
        "[user]\n    name = Test User\n    email = test@memexia.local\n",
    );
    std::fs::write(&config_path, config_content).ok();
}
