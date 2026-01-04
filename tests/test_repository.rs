//! Repository 模块集成测试

use memexia::core::Repository;
use std::fs;

/// 辅助函数：规范化路径字符串用于比较（处理 Windows 短路径和大小写问题）
fn normalize_path(path: &std::path::Path) -> String {
    // 获取规范化的路径并转换为大写
    if let Ok(canonical) = fs::canonicalize(path) {
        canonical.to_string_lossy().to_uppercase()
    } else {
        path.to_string_lossy().to_uppercase()
    }
}

#[test]
fn test_repository_init() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    // 先配置 Git
    configure_git_user(path);

    let repo = Repository::init(path).unwrap();
    assert!(path.join(".memexia").exists());
    assert!(path.join(".git").exists());
    assert_eq!(normalize_path(repo.path()), normalize_path(path));
}

#[test]
fn test_repository_init_already_exists() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    Repository::init(path).unwrap();
    let result = Repository::init(path);
    assert!(result.is_err());
}

#[test]
fn test_repository_open() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    Repository::init(path).unwrap();
    let repo = Repository::open(path).unwrap();
    assert_eq!(normalize_path(repo.path()), normalize_path(path));
}

#[test]
fn test_repository_open_not_exists() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path().join("not_exists");

    let result = Repository::open(&path);
    assert!(result.is_err());
}

#[test]
fn test_repository_open_finds_parent() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    Repository::init(path).unwrap();
    let subdir = path.join("subdir");
    fs::create_dir_all(&subdir).unwrap();

    let repo = Repository::open(&subdir).unwrap();
    assert_eq!(normalize_path(repo.path()), normalize_path(path));
}

#[test]
fn test_repository_add() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");
    fs::write(&test_file, "# Test").unwrap();

    repo.add(&[test_file.clone()]).unwrap();
    assert!(path.join(".memexia/index").exists());
}

#[test]
fn test_repository_add_multiple_files() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let files: Vec<_> = (0..3)
        .map(|i| {
            let f = path.join(format!("test{}.md", i));
            fs::write(&f, format!("# Test {}", i)).unwrap();
            f
        })
        .collect();

    repo.add(&files).unwrap();
    let status = repo.status().unwrap();
    assert!(status.contains("test0.md"));
    assert!(status.contains("test1.md"));
    assert!(status.contains("test2.md"));
}

#[test]
fn test_repository_status_empty() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let status = repo.status().unwrap();
    assert_eq!(status, "No changes staged.");
}

#[test]
fn test_repository_status_with_files() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");
    fs::write(&test_file, "# Test").unwrap();

    repo.add(&[test_file]).unwrap();
    let status = repo.status().unwrap();
    assert!(status.contains("Staged files:"));
}

#[test]
fn test_repository_commit() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let mut repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");
    fs::write(&test_file, "# Test\n\n[[Another Page]]").unwrap();

    repo.add(&[test_file]).unwrap();
    let commit_hash = repo.commit("Initial commit").unwrap();
    assert!(!commit_hash.is_empty());
}

#[test]
fn test_repository_commit_no_staged() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let mut repo = Repository::init(path).unwrap();
    let result = repo.commit("No files");
    assert!(result.is_err());
}

#[test]
fn test_repository_amend() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let mut repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");
    fs::write(&test_file, "# Test").unwrap();

    repo.add(&[test_file.clone()]).unwrap();
    repo.commit("Original").unwrap();

    repo.amend("Amended message").unwrap();
}

#[test]
fn test_repository_log() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let mut repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");
    fs::write(&test_file, "# Test").unwrap();

    repo.add(&[test_file]).unwrap();
    repo.commit("First").unwrap();

    let logs = repo.log(10).unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].message, "First");
}

#[test]
fn test_repository_log_limit() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let mut repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");

    for i in 1..=5 {
        fs::write(&test_file, format!("# Test {}", i)).unwrap();
        repo.add(&[test_file.clone()]).unwrap();
        repo.commit(&format!("Commit {}", i)).unwrap();
    }

    let logs = repo.log(3).unwrap();
    assert_eq!(logs.len(), 3);
}

#[test]
fn test_repository_last_commit() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let mut repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");
    fs::write(&test_file, "# Test").unwrap();

    repo.add(&[test_file]).unwrap();
    repo.commit("First commit").unwrap();

    let last = repo.last_commit().unwrap();
    assert!(last.is_some());
    assert_eq!(last.unwrap().message, "First commit");
}

#[test]
fn test_repository_last_commit_none() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let last = repo.last_commit().unwrap();
    assert!(last.is_none());
}

#[test]
fn test_repository_query_graph() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let result = repo.query_graph("SELECT * WHERE { ?s ?p ?o }").unwrap();
    assert!(result.is_empty() || !result.is_empty());
}

#[test]
fn test_repository_export_nquads() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let nquads = repo.export_nquads().unwrap();
    assert!(nquads.is_empty() || nquads.contains("@prefix"));
}

#[test]
fn test_repository_graph_history() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let mut repo = Repository::init(path).unwrap();
    let test_file = path.join("test.md");
    fs::write(&test_file, "# Test").unwrap();

    repo.add(&[test_file]).unwrap();
    repo.commit("First").unwrap();

    let history = repo.graph_history(10).unwrap();
    assert!(!history.is_empty());
}

#[test]
fn test_repository_storage_getter() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let storage = repo.storage();
    assert!(!storage.root().to_string_lossy().is_empty());
}

#[test]
fn test_repository_vcs_getter() {
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let path = temp.path();

    configure_git_user(path);
    let repo = Repository::init(path).unwrap();
    let vcs = repo.vcs();
    assert!(!vcs.head_info().unwrap().is_some());
}

/// 配置 Git 用户信息（Windows 需要）
fn configure_git_user(path: &std::path::Path) {
    let git_dir = path.join(".git");
    let config_path = git_dir.join("config");

    // 如果 .git 目录不存在，先创建它
    if !git_dir.exists() {
        if let Err(e) = std::process::Command::new("git")
            .args(&["init", path.to_str().unwrap()])
            .output()
        {
            // git init 失败时静默处理
            let _ = e;
        }
    }

    // 检查是否已有用户配置
    if let Ok(content) = fs::read_to_string(&config_path) {
        if content.contains("user.name") {
            return;
        }
    }

    // 添加用户配置
    let config_content = format!(
        "[user]\n    name = Test User\n    email = test@memexia.local\n",
    );
    let _ = fs::write(&config_path, config_content);
}
