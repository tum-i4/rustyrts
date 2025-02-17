use rustc_span::source_map::{FileLoader, RealFileLoader};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{atomic::AtomicBool, Arc};

static TEST_RUNNER_INSERTED: AtomicBool = AtomicBool::new(false);
static EXTERN_CRATE_INSERTED: AtomicBool = AtomicBool::new(false);

pub struct TestRunnerFileLoaderProxy {
    pub(crate) delegate: InstrumentationFileLoaderProxy,
}

impl FileLoader for TestRunnerFileLoaderProxy {
    fn file_exists(&self, path: &std::path::Path) -> bool {
        self.delegate.file_exists(path)
    }

    fn read_file(&self, path: &std::path::Path) -> std::io::Result<String> {
        let content = self.delegate.read_file(path)?;

        if !TEST_RUNNER_INSERTED.load(SeqCst) {
            TEST_RUNNER_INSERTED.store(true, SeqCst);

            let extended_content = format!(
                "{content}

                #[allow(unused_extern_crates)]
                extern crate test as rustyrts_test;
                
                #[link(name = \"rustyrts_dynamic_runner\")]
                #[allow(improper_ctypes)]
                #[allow(dead_code)]
                extern {{
                    fn rustyrts_runner(tests: &[&rustyrts_test::TestDescAndFn]);
                }}

                #[allow(unsafe_code)]
                #[allow(dead_code)]
                fn rustyrts_runner_wrapper(tests: &[&rustyrts_test::TestDescAndFn]) 
                {{ 
                    unsafe {{ rustyrts_runner(tests); }}
                }}"
            )
            .to_string();

            Ok(extended_content)
        } else {
            Ok(content)
        }
    }

    fn read_binary_file(&self, path: &std::path::Path) -> std::io::Result<Arc<[u8]>> {
        self.delegate.read_binary_file(path)
    }
}

pub struct InstrumentationFileLoaderProxy {
    pub delegate: RealFileLoader,
}

impl FileLoader for InstrumentationFileLoaderProxy {
    fn file_exists(&self, path: &std::path::Path) -> bool {
        self.delegate.file_exists(path)
    }

    fn read_file(&self, path: &std::path::Path) -> std::io::Result<String> {
        let content = self.delegate.read_file(path)?;
        if !EXTERN_CRATE_INSERTED.load(SeqCst) {
            EXTERN_CRATE_INSERTED.store(true, SeqCst);

            let extended_content = format!(
                "{content}

                #[allow(unused_extern_crates)]
                extern crate rustyrts_dynamic_rlib;"
            )
            .to_string();

            Ok(extended_content)
        } else {
            Ok(content)
        }
    }

    fn read_binary_file(&self, path: &std::path::Path) -> std::io::Result<Arc<[u8]>> {
        self.delegate.read_binary_file(path)
    }
}
