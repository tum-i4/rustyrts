use cargo::core::Target;
use lazy_static::lazy_static;
use regex::Regex;
use std::hash::{DefaultHasher, Hash, Hasher};

lazy_static! {
    static ref RE_SEMVER: Regex = Regex::new(r"(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)(?:-(?P<prerelease>(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+(?P<buildmetadata>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?").unwrap();
}

pub(crate) fn get_target_hash(target: &Target) -> String {
    let cropped_path = target
        .src_path()
        .path()
        .and_then(|p| p.to_str())
        .map(|p| RE_SEMVER.replace_all(&p, "").to_string());

    let mut hasher = DefaultHasher::new();
    target.kind().hash(&mut hasher);
    // target.name().hash(&mut hasher);
    target.binary_filename().hash(&mut hasher);
    cropped_path.hash(&mut hasher);
    // target.tested().hash(&mut hasher);
    // target.benched().hash(&mut hasher);
    // target.documented().hash(&mut hasher);
    // target.doctested().hash(&mut hasher);
    // target.harness().hash(&mut hasher);
    // target.for_host().hash(&mut hasher);
    // target.proc_macro().hash(&mut hasher);
    // target.edition().hash(&mut hasher);

    format!("{:016x}", hasher.finish())
}
