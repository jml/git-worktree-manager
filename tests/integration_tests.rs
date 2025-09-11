use git2::{Repository, Signature};
use gwm::git::{GitRepository, SystemGitClient};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a temporary bare repository with an initial commit
fn setup_bare_repo_with_commit() -> (TempDir, String) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let repo_path = temp_dir.path().join("test-repo");

    // Initialize bare repository
    let _repo = Repository::init_bare(&repo_path).expect("Failed to init bare repo");

    // Create a temporary non-bare repository to make the initial commit
    let temp_worktree = TempDir::new().expect("Failed to create temp worktree");
    let clone_path = temp_worktree.path().join("clone");
    let clone_repo = Repository::clone(repo_path.to_str().unwrap(), &clone_path)
        .expect("Failed to clone bare repo");

    // Create a test file and make initial commit
    let test_file_path = clone_path.join("README.md");
    fs::write(&test_file_path, "# Test Repository\n").expect("Failed to write test file");

    // Stage and commit the file
    let mut index = clone_repo.index().expect("Failed to get index");
    index
        .add_path(Path::new("README.md"))
        .expect("Failed to add file to index");
    index.write().expect("Failed to write index");

    let signature =
        Signature::now("Test User", "test@example.com").expect("Failed to create signature");
    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = clone_repo.find_tree(tree_id).expect("Failed to find tree");

    clone_repo
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )
        .expect("Failed to create initial commit");

    // Push to the bare repository
    let mut remote = clone_repo
        .find_remote("origin")
        .expect("Failed to find remote");
    remote
        .push(&["refs/heads/main:refs/heads/main"], None)
        .expect("Failed to push to bare repo");

    (temp_dir, repo_path.to_string_lossy().to_string())
}

#[test]
fn test_add_worktree_creates_branch_successfully() {
    let (_temp_dir, repo_path) = setup_bare_repo_with_commit();

    // Create GitRepository instance
    let git_repo =
        GitRepository::new(&repo_path, SystemGitClient).expect("Failed to open repository");

    // Create a temporary directory for the worktree
    let worktree_dir = TempDir::new().expect("Failed to create worktree temp dir");
    let worktree_path = worktree_dir.path().join("test-branch");

    // This should succeed but currently fails with the commit ownership error
    let result = git_repo.add_worktree(
        "test-branch",
        worktree_path.to_str().unwrap(),
        Some("main"),
        false,
    );

    // Assert that the worktree was created successfully
    match result {
        Ok(()) => {
            // Verify the worktree directory exists
            assert!(worktree_path.exists(), "Worktree directory should exist");

            // Verify the branch was created and checked out properly
            let worktree_repo = Repository::open(&worktree_path)
                .expect("Should be able to open worktree repository");

            let head = worktree_repo.head().expect("Should have a HEAD reference");
            assert!(head.is_branch(), "HEAD should point to a branch");

            let branch_name = head.shorthand().expect("Should have a branch name");
            assert_eq!(
                branch_name, "test-branch",
                "Should be on the correct branch"
            );
        }
        Err(e) => {
            panic!("add_worktree failed: {}", e);
        }
    }
}

#[test]
fn test_add_worktree_with_existing_branch() {
    let (_temp_dir, repo_path) = setup_bare_repo_with_commit();

    let git_repo =
        GitRepository::new(&repo_path, SystemGitClient).expect("Failed to open repository");

    let worktree_dir = TempDir::new().expect("Failed to create worktree temp dir");
    let worktree_path = worktree_dir.path().join("existing-main");

    // This should work since main branch already exists
    let result = git_repo.add_worktree("main", worktree_path.to_str().unwrap(), None, true);

    match &result {
        Ok(()) => {
            assert!(worktree_path.exists(), "Worktree directory should exist");
        }
        Err(e) => {
            println!("Error creating worktree from existing branch: {}", e);
            panic!(
                "Should be able to create worktree from existing branch: {}",
                e
            );
        }
    }
}

#[test]
fn test_add_worktree_fails_when_path_exists() {
    let (_temp_dir, repo_path) = setup_bare_repo_with_commit();

    let git_repo =
        GitRepository::new(&repo_path, SystemGitClient).expect("Failed to open repository");

    let worktree_dir = TempDir::new().expect("Failed to create worktree temp dir");
    let worktree_path = worktree_dir.path().join("existing-path");

    // Create the directory first
    fs::create_dir_all(&worktree_path).expect("Failed to create directory");

    // This should fail because the path already exists
    let result = git_repo.add_worktree(
        "test-branch",
        worktree_path.to_str().unwrap(),
        Some("main"),
        false,
    );

    assert!(
        result.is_err(),
        "Should fail when target path already exists"
    );
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_add_remove_add_sequence_works_with_reuse() {
    // Tests that add -> remove -> add works with --reuse flag
    let (_temp_dir, repo_path) = setup_bare_repo_with_commit();

    let git_repo =
        GitRepository::new(&repo_path, SystemGitClient).expect("Failed to open repository");

    // Create temporary directories for worktrees
    let worktree_dir1 = TempDir::new().expect("Failed to create worktree temp dir");
    let worktree_path1 = worktree_dir1.path().join("new-tree");

    let worktree_dir2 = TempDir::new().expect("Failed to create worktree temp dir");
    let worktree_path2 = worktree_dir2.path().join("new-tree");

    // Step 1: Add worktree - should succeed
    let add_result1 = git_repo.add_worktree(
        "new-tree",
        worktree_path1.to_str().unwrap(),
        Some("main"),
        false,
    );

    match add_result1 {
        Ok(()) => {
            assert!(
                worktree_path1.exists(),
                "First worktree directory should exist"
            );

            // Verify the branch was created
            let worktree_repo = Repository::open(&worktree_path1)
                .expect("Should be able to open first worktree repository");
            let head = worktree_repo.head().expect("Should have a HEAD reference");
            let branch_name = head.shorthand().expect("Should have a branch name");
            assert_eq!(branch_name, "new-tree", "Should be on the new-tree branch");
        }
        Err(e) => panic!("First add_worktree should succeed but failed: {}", e),
    }

    // Step 2: Remove worktree - should succeed
    let remove_result = git_repo.remove_worktree("new-tree");
    assert!(
        remove_result.is_ok(),
        "remove_worktree should succeed: {:?}",
        remove_result
    );

    // Verify worktree directory is gone
    // Note: We expect the first directory to be removed, but we are creating in a different location
    // for the second add attempt to avoid path conflicts

    // Step 3: Add worktree again with same branch name - currently fails but should work
    let add_result2 = git_repo.add_worktree(
        "new-tree",
        worktree_path2.to_str().unwrap(),
        Some("main"),
        true, // Use --reuse to allow reusing existing branch
    );

    match add_result2 {
        Ok(()) => {
            // This is what we want to happen
            assert!(
                worktree_path2.exists(),
                "Second worktree directory should exist"
            );

            let worktree_repo = Repository::open(&worktree_path2)
                .expect("Should be able to open second worktree repository");
            let head = worktree_repo.head().expect("Should have a HEAD reference");
            let branch_name = head.shorthand().expect("Should have a branch name");
            assert_eq!(branch_name, "new-tree", "Should be on the new-tree branch");
        }
        Err(e) => {
            // This should not happen with --reuse flag
            panic!(
                "Second add_worktree with --reuse should succeed but failed with: {}",
                e
            );
        }
    }
}

#[test]
fn test_reuse_flag_prevents_failure_with_existing_branch() {
    // Tests that --reuse flag allows reusing existing branches
    let (_temp_dir, repo_path) = setup_bare_repo_with_commit();

    let git_repo =
        GitRepository::new(&repo_path, SystemGitClient).expect("Failed to open repository");

    // Create temporary directories for worktrees
    let worktree_dir1 = TempDir::new().expect("Failed to create worktree temp dir");
    let worktree_path1 = worktree_dir1.path().join("feature-branch");

    let worktree_dir2 = TempDir::new().expect("Failed to create worktree temp dir");
    let worktree_path2 = worktree_dir2.path().join("feature-branch-2");

    // Step 1: Create initial worktree
    let add_result1 = git_repo.add_worktree(
        "feature-branch",
        worktree_path1.to_str().unwrap(),
        Some("main"),
        false,
    );
    assert!(
        add_result1.is_ok(),
        "First add_worktree should succeed: {:?}",
        add_result1
    );

    // Step 2: Remove the worktree (leaves branch behind)
    let remove_result = git_repo.remove_worktree("feature-branch");
    assert!(
        remove_result.is_ok(),
        "remove_worktree should succeed: {:?}",
        remove_result
    );

    // Step 3: Try to create worktree without --reuse flag (should fail)
    let add_without_reuse = git_repo.add_worktree(
        "feature-branch",
        worktree_path2.to_str().unwrap(),
        Some("main"),
        false, // No reuse
    );

    assert!(
        add_without_reuse.is_err(),
        "Should fail without --reuse flag"
    );
    let error_msg = add_without_reuse.unwrap_err().to_string();
    assert!(
        error_msg.contains("already exists") && error_msg.contains("--reuse"),
        "Error should mention existing branch and --reuse flag: {}",
        error_msg
    );

    // Step 4: Try again with --reuse flag (should succeed)
    let add_with_reuse = git_repo.add_worktree(
        "feature-branch",
        worktree_path2.to_str().unwrap(),
        Some("main"),
        true, // With reuse
    );

    assert!(
        add_with_reuse.is_ok(),
        "Should succeed with --reuse flag: {:?}",
        add_with_reuse
    );

    // Verify the worktree was created successfully
    assert!(worktree_path2.exists(), "Worktree directory should exist");

    let worktree_repo =
        Repository::open(&worktree_path2).expect("Should be able to open worktree repository");
    let head = worktree_repo.head().expect("Should have a HEAD reference");
    let branch_name = head.shorthand().expect("Should have a branch name");
    assert_eq!(
        branch_name, "feature-branch",
        "Should be on the feature-branch branch"
    );
}
