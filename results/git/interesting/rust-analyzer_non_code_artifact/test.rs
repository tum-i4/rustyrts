#[test]
fn files_are_tidy() {
    let files = sourcegen::list_files(&sourcegen::project_root().join("crates"));

    let mut tidy_docs = TidyDocs::default();
    let mut tidy_marks = TidyMarks::default();
    for path in files {
        let extension = path.extension().unwrap_or_default().to_str().unwrap_or_default();
        match extension {
            "rs" => {
                let text = read_file(&path).unwrap();
                check_todo(&path, &text);
                check_dbg(&path, &text);
                check_test_attrs(&path, &text);
                check_trailing_ws(&path, &text);
                deny_clippy(&path, &text);
                tidy_docs.visit(&path, &text);
                tidy_marks.visit(&path, &text);
            }
            "toml" => {
                let text = read_file(&path).unwrap();
                check_cargo_toml(&path, text);
            }
            _ => (),
        }
    }

    tidy_docs.finish();
    tidy_marks.finish();
}


fn check_dbg(path: &Path, text: &str) {
    let need_dbg = &[
        // This file itself obviously needs to use dbg.
        "slow-tests/tidy.rs",
        // Assists to remove `dbg!()`
        "handlers/remove_dbg.rs",
        // We have .dbg postfix
        "ide_completion/src/completions/postfix.rs",
        "ide_completion/src/completions/keyword.rs",
        "ide_completion/src/tests/proc_macros.rs",
        // The documentation in string literals may contain anything for its own purposes
        "ide_completion/src/lib.rs",
        "ide_db/src/helpers/generated_lints.rs",
        // test for doc test for remove_dbg
        "src/tests/generated.rs",
    ];
    if need_dbg.iter().any(|p| path.ends_with(p)) {
        return;
    }
    if text.contains("dbg!") {
        panic!(
            "\ndbg! macros should not be committed to the master branch,\n\
             {}\n",
            path.display(),
        )
    }
}
