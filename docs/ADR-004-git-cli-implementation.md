# ADR-004: Git CLI Implementation

**Status:** Accepted (Revised 2026-01-12)

**Context:** We need a way to interact with Git repositories for configuration management, including clone, pull, checkout, and other operations.

## Decision

Use the system's `git` command-line interface via `std::process::Command` instead of Rust libraries (gix or git2).

## Rationale

### Advantages of Git CLI

1. **Maximum Compatibility**
   - Works with any Git repository without implementation quirks
   - Handles all edge cases that the Git core team has already solved
   - No compatibility issues with different Git server implementations

2. **Simplicity**
   - No additional Rust dependencies required
   - Straightforward implementation using `std::process::Command`
   - Easy to understand and maintain

3. **Maturity**
   - Git CLI is extremely stable and battle-tested
   - Decades of production use and bug fixes
   - Well-documented behavior

4. **Debugging**
   - Easy to debug - same commands work in terminal
   - Clear error messages from git
   - Can reproduce issues manually

5. **No C Dependencies**
   - Does not require C toolchain for building
   - Does not require libgit2 system library
   - Simpler deployment (only requires git binary)

6. **Async Integration**
   - Works well with Tokio's `spawn_blocking`
   - Prevents blocking the async runtime
   - Simple mental model

## Alternatives Considered

### Option 1: gix (gitoxide) - Pure Rust

**Pros:**
- Pure Rust implementation
- No system dependencies
- Type-safe API
- Potentially better performance for some operations

**Cons:**
- API still evolving (not 1.0 yet)
- Potential compatibility issues with edge cases
- Less mature than Git CLI
- Larger binary size
- More complex error handling

**Verdict:** Rejected due to API stability concerns and potential edge case handling.

### Option 2: git2-rs (libgit2 bindings)

**Pros:**
- Mature and widely used
- Rich Rust API
- Good documentation
- Proven in production

**Cons:**
- Requires C toolchain for building
- Requires libgit2 system library
- Complex cross-compilation
- Potential version mismatches between libgit2 versions
- Harder to debug than CLI

**Verdict:** Rejected due to C dependencies and deployment complexity.

### Option 3: Git CLI (chosen)

**Pros:**
- Maximum compatibility
- Simple implementation
- Easy debugging
- No additional dependencies
- Mature and stable

**Cons:**
- Requires git to be installed on system
- Text parsing instead of structured API
- Slightly less control over internals

**Verdict:** Accepted - best balance of simplicity, compatibility, and maintainability.

## Trade-offs Accepted

1. **System Dependency**
   - Git must be installed on the system
   - **Mitigation:** Git is ubiquitous on server systems; easy to include in Docker images

2. **Text Parsing**
   - Must parse command output as text
   - **Mitigation:** Git output is stable and well-structured; only parse simple outputs (commit SHAs, status)

3. **Less Control**
   - Cannot fine-tune internal Git operations
   - **Mitigation:** Default Git behavior is appropriate for our use case

## Implementation Details

### How Operations are Executed

```rust
// Blocking operation in spawn_blocking
tokio::task::spawn_blocking(move || {
    let output = Command::new("git")
        .args(["clone", "--depth", "1", uri])
        .arg(local_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ConfigSourceError::git(format!("Clone failed: {}", stderr)));
    }

    Ok(())
}).await
```

### Operations Implemented

- `git clone --depth 1` - Clone repository
- `git fetch --all --prune` - Fetch updates
- `git checkout` - Switch branches/tags
- `git rev-parse HEAD` - Get current commit
- `git branch -a` - List branches
- `git tag -l` - List tags

### Error Handling

- Check exit status of git commands
- Parse stderr for error messages
- Map to domain-specific error types
- Provide context for debugging

## Consequences

### Positive

- Simple, maintainable implementation
- Easy onboarding for new developers
- Predictable behavior
- Easy to test (mock git commands in tests)
- Smaller binary size (no git library)

### Negative

- Requires git installation
- Text output parsing
- Cannot use advanced libgit2 features (if needed in future)

### Neutral

- Performance is adequate for our use case
- Could switch to library in future if needed (abstraction via ConfigSource trait)

## Future Considerations

If requirements change (e.g., need for embedded git, advanced features), we can:

1. Keep Git CLI as default implementation
2. Add optional gix-based implementation behind feature flag
3. Abstract via ConfigSource trait allows swapping implementations

## References

- Git CLI Documentation: https://git-scm.com/docs
- vortex-git implementation: `crates/vortex-git/src/repository/git_ops.rs`
- ConfigSource abstraction: `crates/vortex-git/src/source/traits.rs`

## Revision History

- 2026-01-12: Initial documentation of actual implementation
- Original planning mentioned gix but implementation chose Git CLI for production reliability
