use core::str;
use git2::{Repository, StatusOptions};
use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    // In contrast to git describe, we don't want to show a tag name, but always the short sha and
    // dirty status as a useful addition to the version number in CARGO_PKG_VERSION.
    let repo = Repository::discover(env::var("CARGO_MANIFEST_DIR")?)?;
    let head_obj = repo.revparse_single("HEAD")?;
    let short_sha_buf = head_obj.short_id()?;
    let short_sha = str::from_utf8(&short_sha_buf)?;
    let mut status_options = StatusOptions::default();
    // let _ = status_options.include_untracked(true);
    let statuses = repo.statuses(Some(&mut status_options))?;
    let dirty = statuses.iter().any(|st| !st.status().is_ignored());
    let dirty_str = if dirty { "+" } else { "" };
    println!("cargo::rustc-env=GIT_SHORT_SHA={short_sha}{dirty_str}");

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=.git/");
    println!("cargo::rerun-if-changed=.git/HEAD");

    Ok(())
}
