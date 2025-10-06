pub fn is_repo(_root: &std::path::Path) -> bool { false }
pub fn commit_all(_root:&std::path::Path, _message:&str) -> anyhow::Result<String> { Ok(String::new()) }
pub fn tag(_root:&std::path::Path, _name:&str, _commit:&str) -> anyhow::Result<()> { Ok(()) }
pub fn rollback_last(_root:&std::path::Path) -> anyhow::Result<()> { Ok(()) }
