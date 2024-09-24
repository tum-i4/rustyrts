use cargo::core::Target;
use std::hash::{DefaultHasher, Hash, Hasher};

pub(crate) fn get_target_hash(target: &Target) -> String {
    let mut hasher = DefaultHasher::new();
    target.kind().hash(&mut hasher);
    // target.name().hash(&mut hasher);
    target.binary_filename().hash(&mut hasher);
    target.src_path().path().hash(&mut hasher);
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
