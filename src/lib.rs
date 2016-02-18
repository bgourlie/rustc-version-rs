// Copyright 2016 rustc-version-rs developers
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![warn(missing_docs)]

//! Simple library for getting the version information of a `rustc`
//! compiler.
//!
//! This calls `$RUSTC --version` and parses the output, falling
//! back to `rustc` if `$RUSTC` is not set.
//!
//! # Example
//!
//! ```rust
//! // This could be a cargo build script
//!
//! extern crate rustc_version;
//! use rustc_version::{version, version_meta, Channel, Version};
//!
//! fn main() {
//!     // Assert we haven't travelled back in time
//!     assert!(version().unwrap().major >= 1);
//!
//!     // Set cfg flags depending on release channel
//!     match version_meta().unwrap().channel {
//!         Channel::Stable => {
//!             println!("cargo:rustc-cfg=RUSTC_IS_STABLE");
//!         }
//!         Channel::Beta => {
//!             println!("cargo:rustc-cfg=RUSTC_IS_BETA");
//!         }
//!         Channel::Nightly => {
//!             println!("cargo:rustc-cfg=RUSTC_IS_NIGHTLY");
//!         }
//!         Channel::Dev => {
//!             println!("cargo:rustc-cfg=RUSTC_IS_DEV");
//!         }
//!     }
//!
//!     // Check for a minimum version
//!     if version().unwrap() >= Version::parse("1.4.0").unwrap() {
//!         println!("cargo:rustc-cfg=compiler_has_important_bugfix");
//!     }
//! }
//! ```

extern crate semver;
use semver::Identifier;
use std::process::Command;
use std::{env, str};
use std::ffi::OsString;

// Convenience re-export to allow version comparison without needing to add
// semver crate.
pub use semver::Version;

mod errors;
pub use errors::{Error, Result};

/// Release channel of the compiler.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Channel {
    /// Development release channel
    Dev,
    /// Nightly release channel
    Nightly,
    /// Beta release channel
    Beta,
    /// Stable release channel
    Stable,
}

/// Rustc version plus metada like git short hash and build date.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VersionMeta {
    /// Version of the compiler
    pub semver: Version,

    /// Git short hash of the build of the compiler
    pub commit_hash: Option<String>,

    /// Commit date of the compiler
    pub commit_date: Option<String>,

    /// Build date of the compiler; this was removed between Rust 1.0.0 and 1.1.0.
    pub build_date: Option<String>,

    /// Release channel of the compiler
    pub channel: Channel,

    /// Host target triple of the compiler
    pub host: String,

    /// Short version string of the compiler
    pub short_version_string: String,
}

/// Returns the `rustc` SemVer version.
pub fn version() -> Result<Version> {
    Ok(try!(version_meta()).semver)
}

/// Returns the `rustc` SemVer version and additional metadata
/// like the git short hash and build date.
pub fn version_meta() -> Result<VersionMeta> {
    let cmd = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));

    let out = match Command::new(&cmd).arg("-vV").output() {
        Err(e) => return Err(Error::CouldNotExecuteCommand(e)),
        Ok(out) => out,
    };

    let out = try!(str::from_utf8(&out.stdout));

    version_meta_for(out)
}

/// Parses a "rustc -vV" output string and returns
/// the SemVer version and additional metadata
/// like the git short hash and build date.
pub fn version_meta_for(verbose_version_string: &str) -> Result<VersionMeta> {
    let out: Vec<_> = verbose_version_string.lines().collect();

    if !(out.len() == 6 || out.len() == 7) {
        return Err(Error::UnexpectedVersionFormat);
    }

    let short_version_string = out[0];

    fn expect_prefix<'a>(line: &'a str, prefix: &str) -> Result<&'a str> {
        match line.starts_with(prefix) {
            true => Ok(&line[prefix.len()..]),
            false => Err(Error::UnexpectedVersionFormat),
        }
    }

    let commit_hash = match try!(expect_prefix(out[2], "commit-hash: ")) {
        "unknown" => None,
        hash => Some(hash.to_owned()),
    };

    let commit_date = match try!(expect_prefix(out[3], "commit-date: ")) {
        "unknown" => None,
        hash => Some(hash.to_owned()),
    };

    // Handle that the build date may or may not be present.
    let mut idx = 4;
    let mut build_date = None;
    if out[idx].starts_with("build-date") {
        build_date = match try!(expect_prefix(out[idx], "build-date: ")) {
            "unknown" => None,
            s => Some(s.to_owned()),
        };
        idx = idx + 1;
    }

    let host = try!(expect_prefix(out[idx], "host: "));
    idx = idx + 1;
    let release = try!(expect_prefix(out[idx], "release: "));

    let semver: Version = try!(release.parse());

    let channel = if semver.pre.is_empty() {
        Channel::Stable
    } else {
        match semver.pre[0] {
            Identifier::AlphaNumeric(ref s)
                if s == "dev" => Channel::Dev,
            Identifier::AlphaNumeric(ref s)
                if s == "beta" => Channel::Beta,
            Identifier::AlphaNumeric(ref s)
                if s == "nightly" => Channel::Nightly,
            ref x => return Err(Error::UnknownPreReleaseTag(x.clone())),
        }
    };

    Ok(VersionMeta {
        semver: semver,
        commit_hash: commit_hash,
        commit_date: commit_date,
        build_date: build_date,
        channel: channel,
        host: host.into(),
        short_version_string: short_version_string.into(),
    })
}

#[test]
fn smoketest() {
    let v = version().unwrap();
    assert!(v.major >= 1);

    let v = version_meta().unwrap();
    assert!(v.semver.major >= 1);

    assert!(version().unwrap() >= Version::parse("1.0.0").unwrap());
}

#[test]
fn parse_unexpected() {
    let res = version_meta_for(
"rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)
binary: rustc
commit-hash: a59de37e99060162a2674e3ff45409ac73595c0e
commit-date: 2015-05-13
rust-birthday: 2015-05-14
host: x86_64-unknown-linux-gnu
release: 1.0.0");

    assert!(match res {
        Err(Error::UnexpectedVersionFormat) => true,
        _ => false,
    });

}

#[test]
fn parse_1_0_0() {
    let version = version_meta_for(
"rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)
binary: rustc
commit-hash: a59de37e99060162a2674e3ff45409ac73595c0e
commit-date: 2015-05-13
build-date: 2015-05-14
host: x86_64-unknown-linux-gnu
release: 1.0.0").unwrap();

    assert_eq!(version.semver, Version::parse("1.0.0").unwrap());
    assert_eq!(version.commit_hash, Some("a59de37e99060162a2674e3ff45409ac73595c0e".into()));
    assert_eq!(version.commit_date, Some("2015-05-13".into()));
    assert_eq!(version.build_date, Some("2015-05-14".into()));
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(version.short_version_string, "rustc 1.0.0 (a59de37e9 2015-05-13) (built 2015-05-14)");
}


#[test]
fn parse_unknown() {
    let version = version_meta_for(
"rustc 1.3.0
binary: rustc
commit-hash: unknown
commit-date: unknown
host: x86_64-unknown-linux-gnu
release: 1.3.0").unwrap();

    assert_eq!(version.semver, Version::parse("1.3.0").unwrap());
    assert_eq!(version.commit_hash, None);
    assert_eq!(version.commit_date, None);
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(version.short_version_string, "rustc 1.3.0");
}

#[test]
fn parse_nightly() {
    let version = version_meta_for(
"rustc 1.5.0-nightly (65d5c0833 2015-09-29)
binary: rustc
commit-hash: 65d5c083377645a115c4ac23a620d3581b9562b6
commit-date: 2015-09-29
host: x86_64-unknown-linux-gnu
release: 1.5.0-nightly").unwrap();

    assert_eq!(version.semver, Version::parse("1.5.0-nightly").unwrap());
    assert_eq!(version.commit_hash, Some("65d5c083377645a115c4ac23a620d3581b9562b6".into()));
    assert_eq!(version.commit_date, Some("2015-09-29".into()));
    assert_eq!(version.channel, Channel::Nightly);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(version.short_version_string, "rustc 1.5.0-nightly (65d5c0833 2015-09-29)");
}

#[test]
fn parse_stable() {
    let version = version_meta_for(
"rustc 1.3.0 (9a92aaf19 2015-09-15)
binary: rustc
commit-hash: 9a92aaf19a64603b02b4130fe52958cc12488900
commit-date: 2015-09-15
host: x86_64-unknown-linux-gnu
release: 1.3.0").unwrap();

    assert_eq!(version.semver, Version::parse("1.3.0").unwrap());
    assert_eq!(version.commit_hash, Some("9a92aaf19a64603b02b4130fe52958cc12488900".into()));
    assert_eq!(version.commit_date, Some("2015-09-15".into()));
    assert_eq!(version.channel, Channel::Stable);
    assert_eq!(version.host, "x86_64-unknown-linux-gnu");
    assert_eq!(version.short_version_string, "rustc 1.3.0 (9a92aaf19 2015-09-15)");
}
