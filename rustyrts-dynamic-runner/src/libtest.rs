#![allow(dead_code)]
#![allow(unused_variables)]

//######################################################################################################################
// The content of this file is taken from libtest in rust-lang/rust
//
// If not stated otherwise, only the visibility has been modified
//
//######################################################################################################################

use serde::{Deserialize, Serialize};
use std::any::Any;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::time::Duration;
use std::{error, fmt};
use std::{fs::File, io};
use test::test::{MetricMap, TestTimeOptions};
use test::TestFn::{StaticBenchFn, StaticTestFn};
use test::{test::TestExecTime, TestDesc, TestName};
use test::{NamePadding, Options, ShouldPanic, TestDescAndFn, TestOpts, TestType};

//######################################################################################################################
// From types.rs
// Source: https://github.com/rust-lang/rust/blob/f421586eed77de266a3f99ffa8a5687b7d2d893c/library/test/src/types.rs#L15
// Changes: prefix Custom , derive Serialize and Deserialize

/// Type of the test according to the [rust book](https://doc.rust-lang.org/cargo/guide/tests.html)
/// conventions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CustomTestType {
    /// Unit-tests are expected to be in the `src` folder of the crate.
    UnitTest,
    /// Integration-style tests are expected to be in the `tests` folder of the crate.
    IntegrationTest,
    /// Doctests are created by the `librustdoc` manually, so it's a different type of test.
    DocTest,
    /// Tests for the sources that don't follow the project layout convention
    /// (e.g. tests in raw `main.rs` compiled by calling `rustc --test` directly).
    Unknown,
}

// Source: https://github.com/rust-lang/rust/blob/f421586eed77de266a3f99ffa8a5687b7d2d893c/library/test/src/types.rs#L28
// Changes: prefix Custom , derive Serialize and Deserialize

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CustomNamePadding {
    PadNone,
    PadOnRight,
}

// The name of a test. By convention this follows the rules for rust
// paths; i.e., it should be a series of identifiers separated by double
// colons. This way if some test runner wants to arrange the tests
// hierarchically it may.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum CustomTestName {
    StaticTestName(&'static str),
    DynTestName(String),
    AlignedTestName(Cow<'static, str>, CustomNamePadding),
}

// Source: https://github.com/rust-lang/rust/blob/f421586eed77de266a3f99ffa8a5687b7d2d893c/library/test/src/types.rs#L82
// Changes: prefix Custom , derive Serialize and Deserialize, commented out some fields

// The definition of a single test. A test runner will run a list of
// these.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomTestDesc {
    pub name: CustomTestName,
    pub ignore: bool,
    pub ignore_message: Option<&'static str>,
       #[cfg(not(bootstrap))]
       pub source_file: &'static str,
       #[cfg(not(bootstrap))]
       pub start_line: usize,
       #[cfg(not(bootstrap))]
       pub start_col: usize,
       #[cfg(not(bootstrap))]
       pub end_line: usize,
       #[cfg(not(bootstrap))]
       pub end_col: usize,
    pub should_panic: CustomShouldPanic,
    pub compile_fail: bool,
    pub no_run: bool,
    pub test_type: CustomTestType,
}

//######################################################################################################################
// From options.rs
// Source: https://github.com/rust-lang/rust/blob/f421586eed77de266a3f99ffa8a5687b7d2d893c/library/test/src/types.rs#L115
// Changes: derive Serailize

/// Whether test is expected to panic or not
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CustomShouldPanic {
    No,
    Yes,
    YesWithMessage(&'static str),
}

//######################################################################################################################
// From lib.rs
// Source: https://github.com/rust-lang/rust/blob/104f4300cfddbd956e32820ef202a732f06ec848/library/test/src/lib.rs#L198
// Changes: access modifier set to crate

// Process exit code to be used to indicate test failures.
pub(crate) const ERROR_EXIT_CODE: i32 = 101;

/// Clones static values for putting into a dynamic vector, which test_main()
/// needs to hand out ownership of tests to parallel test runners.
///
/// This will panic when fed any dynamic tests, because they cannot be cloned.
pub(crate) fn make_owned_test(test: &&TestDescAndFn) -> TestDescAndFn {
    match test.testfn {
        StaticTestFn(f) => TestDescAndFn {
            testfn: StaticTestFn(f),
            desc: test.desc.clone(),
        },
        StaticBenchFn(f) => TestDescAndFn {
            testfn: StaticBenchFn(f),
            desc: test.desc.clone(),
        },
        _ => panic!("non-static tests passed to test::test_main_static"),
    }
}

//######################################################################################################################
// From time.rs
// Source: https://github.com/rust-lang/rust/blob/104f4300cfddbd956e32820ef202a732f06ec848/library/test/src/time.rs#L15

pub(crate) const TEST_WARN_TIMEOUT_S: u64 = 60;

//######################################################################################################################
// From formatters/junit.rs
// Source: https://github.com/rust-lang/rust/blob/0d7ed3ba8445452013dd873dc9abcad41a3d82b2/library/test/src/formatters/junit.rs#L12
// Changes: commented out any code that correspond to OutputLocation::Pretty and colors

pub struct JunitFormatter<T> {
    out: OutputLocation<T>,
    results: Vec<(TestDesc, TestResult, Duration)>,
}

impl<T: Write> JunitFormatter<T> {
    pub(crate) fn new(out: OutputLocation<T>) -> Self {
        Self {
            out,
            results: Vec::new(),
        }
    }

    fn write_message(&mut self, s: &str) -> io::Result<()> {
        assert!(!s.contains('\n'));

        self.out.write_all(s.as_ref())
    }
}

impl<T: Write> OutputFormatter for JunitFormatter<T> {
    //fn write_discovery_start(&mut self) -> io::Result<()> {
    //    Err(io::Error::new(
    //        io::ErrorKind::NotFound,
    //        "Not yet implemented!",
    //    ))
    //}
    //
    //fn write_test_discovered(&mut self, _desc: &TestDesc, _test_type: &str) -> io::Result<()> {
    //    Err(io::Error::new(
    //        io::ErrorKind::NotFound,
    //        "Not yet implemented!",
    //    ))
    //}
    //
    //fn write_discovery_finish(&mut self, _state: &ConsoleTestDiscoveryState) -> io::Result<()> {
    //    Err(io::Error::new(
    //        io::ErrorKind::NotFound,
    //        "Not yet implemented!",
    //    ))
    //}

    fn write_run_start(
        &mut self,
        _test_count: usize,
        _shuffle_seed: Option<u64>,
    ) -> io::Result<()> {
        // We write xml header on run start
        self.write_message("<?xml version=\"1.0\" encoding=\"UTF-8\"?>")
    }

    fn write_test_start(&mut self, _desc: &TestDesc) -> io::Result<()> {
        // We do not output anything on test start.
        Ok(())
    }

    fn write_timeout(&mut self, _desc: &TestDesc) -> io::Result<()> {
        // We do not output anything on test timeout.
        Ok(())
    }

    fn write_result(
        &mut self,
        desc: &TestDesc,
        result: &TestResult,
        exec_time: Option<&TestExecTime>,
        _stdout: &[u8],
        _state: &ConsoleTestState,
    ) -> io::Result<()> {
        // Because the testsuite node holds some of the information as attributes, we can't write it
        // until all of the tests have finished. Instead of writing every result as they come in, we add
        // them to a Vec and write them all at once when run is complete.
        let duration = exec_time.map(|t| t.0).unwrap_or_default();
        self.results.push((desc.clone(), result.clone(), duration));
        Ok(())
    }
    fn write_run_finish(&mut self, state: &ConsoleTestState) -> io::Result<bool> {
        self.write_message("<testsuites>")?;

        self.write_message(&format!(
            "<testsuite name=\"test\" package=\"test\" id=\"0\" \
             errors=\"0\" \
             failures=\"{}\" \
             tests=\"{}\" \
             skipped=\"{}\" \
             >",
            state.failed, state.total, state.ignored
        ))?;
        for (desc, result, duration) in std::mem::take(&mut self.results) {
            let (class_name, test_name) = parse_class_name(&desc);
            match result {
                TestResult::TrIgnored => { /* no-op */ }
                TestResult::TrFailed => {
                    self.write_message(&format!(
                        "<testcase classname=\"{}\" \
                         name=\"{}\" time=\"{}\">",
                        class_name,
                        test_name,
                        duration.as_secs_f64()
                    ))?;
                    self.write_message("<failure type=\"assert\"/>")?;
                    self.write_message("</testcase>")?;
                }

                TestResult::TrFailedMsg(ref m) => {
                    self.write_message(&format!(
                        "<testcase classname=\"{}\" \
                         name=\"{}\" time=\"{}\">",
                        class_name,
                        test_name,
                        duration.as_secs_f64()
                    ))?;
                    self.write_message(&format!("<failure message=\"{m}\" type=\"assert\"/>"))?;
                    self.write_message("</testcase>")?;
                }

                TestResult::TrTimedFail => {
                    self.write_message(&format!(
                        "<testcase classname=\"{}\" \
                         name=\"{}\" time=\"{}\">",
                        class_name,
                        test_name,
                        duration.as_secs_f64()
                    ))?;
                    self.write_message("<failure type=\"timeout\"/>")?;
                    self.write_message("</testcase>")?;
                }

                //TestResult::TrBench(ref b) => {
                //    self.write_message(&format!(
                //        "<testcase classname=\"benchmark::{}\" \
                //         name=\"{}\" time=\"{}\" />",
                //        class_name, test_name, b.ns_iter_summ.sum
                //    ))?;
                //}
                //
                TestResult::TrOk => {
                    self.write_message(&format!(
                        "<testcase classname=\"{}\" \
                         name=\"{}\" time=\"{}\"/>",
                        class_name,
                        test_name,
                        duration.as_secs_f64()
                    ))?;
                }
            }
        }
        self.write_message("<system-out/>")?;
        self.write_message("<system-err/>")?;
        self.write_message("</testsuite>")?;
        self.write_message("</testsuites>")?;

        self.out.write_all(b"\n")?;

        Ok(state.failed == 0)
    }
}

fn parse_class_name(desc: &TestDesc) -> (String, String) {
    match desc.test_type {
        TestType::UnitTest => parse_class_name_unit(desc),
        TestType::DocTest => parse_class_name_doc(desc),
        TestType::IntegrationTest => parse_class_name_integration(desc),
        TestType::Unknown => (String::from("unknown"), String::from(desc.name.as_slice())),
    }
}

fn parse_class_name_unit(desc: &TestDesc) -> (String, String) {
    // Module path => classname
    // Function name => name
    let module_segments: Vec<&str> = desc.name.as_slice().split("::").collect();
    let (class_name, test_name) = match module_segments[..] {
        [test] => (String::from("crate"), String::from(test)),
        [ref path @ .., test] => (path.join("::"), String::from(test)),
        [..] => unreachable!(),
    };
    (class_name, test_name)
}

fn parse_class_name_doc(desc: &TestDesc) -> (String, String) {
    // File path => classname
    // Line # => test name
    let segments: Vec<&str> = desc.name.as_slice().split(" - ").collect();
    let (class_name, test_name) = match segments[..] {
        [file, line] => (String::from(file.trim()), String::from(line.trim())),
        [..] => unreachable!(),
    };
    (class_name, test_name)
}

fn parse_class_name_integration(desc: &TestDesc) -> (String, String) {
    (
        String::from("integration"),
        String::from(desc.name.as_slice()),
    )
}

//######################################################################################################################
// From formatters/terse.rs
// Source: https://github.com/rust-lang/rust/blob/0d7ed3ba8445452013dd873dc9abcad41a3d82b2/library/test/src/formatters/terse.rs#L14
// Changes: commented out any code that correspond to OutputLocation::Pretty and colors

// We insert a '\n' when the output hits 100 columns in quiet mode. 88 test
// result chars leaves 12 chars for a progress count like " 11704/12853".
const QUIET_MODE_MAX_COLUMN: usize = 88;

pub(crate) struct TerseFormatter<T> {
    out: OutputLocation<T>,
    use_color: bool,
    is_multithreaded: bool,
    /// Number of columns to fill when aligning names
    max_name_len: usize,

    test_count: usize,
    total_test_count: usize,
}

impl<T: Write> TerseFormatter<T> {
    pub fn new(
        out: OutputLocation<T>,
        use_color: bool,
        max_name_len: usize,
        is_multithreaded: bool,
    ) -> Self {
        TerseFormatter {
            out,
            use_color,
            max_name_len,
            is_multithreaded,
            test_count: 0,
            total_test_count: 0, // initialized later, when write_run_start is called
        }
    }

    pub fn write_ok(&mut self) -> io::Result<()> {
        self.write_short_result(".") //, term::color::GREEN)
    }

    pub fn write_failed(&mut self) -> io::Result<()> {
        self.write_short_result("F") //, term::color::RED)
    }

    pub fn write_ignored(&mut self) -> io::Result<()> {
        self.write_short_result("i") //), term::color::YELLOW)
    }

    pub fn write_bench(&mut self) -> io::Result<()> {
        self.write_pretty("bench") //, term::color::CYAN)
    }

    pub fn write_short_result(
        &mut self,
        result: &str,
        //color: term::color::Color,
    ) -> io::Result<()> {
        self.write_pretty(result)?; //, color)?;
        if self.test_count % QUIET_MODE_MAX_COLUMN == QUIET_MODE_MAX_COLUMN - 1 {
            // We insert a new line regularly in order to flush the
            // screen when dealing with line-buffered output (e.g., piping to
            // `stamp` in the rust CI).
            let out = format!(" {}/{}\n", self.test_count + 1, self.total_test_count);
            self.write_plain(out)?;
        }

        self.test_count += 1;
        Ok(())
    }

    pub fn write_pretty(
        &mut self,
        word: &str, /* , color: term::color::Color*/
    ) -> io::Result<()> {
        match self.out {
            //OutputLocation::Pretty(ref mut term) => {
            //    if self.use_color {
            //        term.fg(color)?;
            //    }
            //    term.write_all(word.as_bytes())?;
            //    if self.use_color {
            //        term.reset()?;
            //    }
            //    term.flush()
            //}
            OutputLocation::Raw(ref mut stdout) => {
                stdout.write_all(word.as_bytes())?;
                stdout.flush()
            }
        }
    }

    pub fn write_plain<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        let s = s.as_ref();
        self.out.write_all(s.as_bytes())?;
        self.out.flush()
    }

    pub fn write_outputs(&mut self, state: &ConsoleTestState) -> io::Result<()> {
        self.write_plain("\nsuccesses:\n")?;
        let mut successes = Vec::new();
        let mut stdouts = String::new();
        for (f, stdout) in &state.not_failures {
            successes.push(f.name.to_string());
            if !stdout.is_empty() {
                stdouts.push_str(&format!("---- {} stdout ----\n", f.name));
                let output = String::from_utf8_lossy(stdout);
                stdouts.push_str(&output);
                stdouts.push('\n');
            }
        }
        if !stdouts.is_empty() {
            self.write_plain("\n")?;
            self.write_plain(&stdouts)?;
        }

        self.write_plain("\nsuccesses:\n")?;
        successes.sort();
        for name in &successes {
            self.write_plain(&format!("    {name}\n"))?;
        }
        Ok(())
    }

    pub fn write_failures(&mut self, state: &ConsoleTestState) -> io::Result<()> {
        self.write_plain("\nfailures:\n")?;
        let mut failures = Vec::new();
        let mut fail_out = String::new();
        for (f, stdout) in &state.failures {
            failures.push(f.name.to_string());
            if !stdout.is_empty() {
                fail_out.push_str(&format!("---- {} stdout ----\n", f.name));
                let output = String::from_utf8_lossy(stdout);
                fail_out.push_str(&output);
                fail_out.push('\n');
            }
        }
        if !fail_out.is_empty() {
            self.write_plain("\n")?;
            self.write_plain(&fail_out)?;
        }

        self.write_plain("\nfailures:\n")?;
        failures.sort();
        for name in &failures {
            self.write_plain(&format!("    {name}\n"))?;
        }
        Ok(())
    }

    fn write_test_name(&mut self, desc: &TestDesc) -> io::Result<()> {
        let name = desc.padded_name(self.max_name_len, desc.name.padding());
        if let Some(test_mode) = desc.test_mode() {
            self.write_plain(format!("test {name} - {test_mode} ... "))?;
        } else {
            self.write_plain(format!("test {name} ... "))?;
        }

        Ok(())
    }
}

impl<T: Write> OutputFormatter for TerseFormatter<T> {
    //fn write_discovery_start(&mut self) -> io::Result<()> {
    //    Ok(())
    //}
    //
    //fn write_test_discovered(&mut self, desc: &TestDesc, test_type: &str) -> io::Result<()> {
    //    self.write_plain(format!("{}: {test_type}\n", desc.name))
    //}
    //
    //fn write_discovery_finish(&mut self, _state: &ConsoleTestDiscoveryState) -> io::Result<()> {
    //    Ok(())
    //}

    fn write_run_start(&mut self, test_count: usize, shuffle_seed: Option<u64>) -> io::Result<()> {
        self.total_test_count = test_count;
        let noun = if test_count != 1 { "tests" } else { "test" };
        let shuffle_seed_msg = if let Some(shuffle_seed) = shuffle_seed {
            format!(" (shuffle seed: {shuffle_seed})")
        } else {
            String::new()
        };
        self.write_plain(format!("\nrunning {test_count} {noun}{shuffle_seed_msg}\n"))
    }

    fn write_test_start(&mut self, desc: &TestDesc) -> io::Result<()> {
        // Remnants from old libtest code that used the padding value
        // in order to indicate benchmarks.
        // When running benchmarks, terse-mode should still print their name as if
        // it is the Pretty formatter.
        if !self.is_multithreaded && desc.name.padding() == NamePadding::PadOnRight {
            self.write_test_name(desc)?;
        }

        Ok(())
    }

    fn write_result(
        &mut self,
        desc: &TestDesc,
        result: &TestResult,
        _: Option<&TestExecTime>,
        _: &[u8],
        _: &ConsoleTestState,
    ) -> io::Result<()> {
        match *result {
            TestResult::TrOk => self.write_ok(),
            TestResult::TrFailed | TestResult::TrFailedMsg(_) | TestResult::TrTimedFail => {
                self.write_failed()
            }
            TestResult::TrIgnored => self.write_ignored(),
            //TestResult::TrBench(ref bs) => {
            //    if self.is_multithreaded {
            //        self.write_test_name(desc)?;
            //    }
            //    self.write_bench()?;
            //    self.write_plain(format!(": {}\n", fmt_bench_samples(bs)))
            //}
        }
    }

    fn write_timeout(&mut self, desc: &TestDesc) -> io::Result<()> {
        self.write_plain(format!(
            "test {} has been running for over {} seconds\n",
            desc.name, TEST_WARN_TIMEOUT_S
        ))
    }

    fn write_run_finish(&mut self, state: &ConsoleTestState) -> io::Result<bool> {
        if state.options.display_output {
            self.write_outputs(state)?;
        }
        let success = state.failed == 0;
        if !success {
            self.write_failures(state)?;
        }

        self.write_plain("\ntest result: ")?;

        if success {
            // There's no parallelism at this point so it's safe to use color
            self.write_pretty("ok")?; //, term::color::GREEN)?;
        } else {
            self.write_pretty("FAILED")?; //, term::color::RED)?;
        }

        let s = format!(
            ". {} passed; {} failed; {} ignored; {} measured; {} filtered out",
            state.passed, state.failed, state.ignored, state.measured, state.filtered_out
        );

        self.write_plain(s)?;

        if let Some(ref exec_time) = state.exec_time {
            let time_str = format!("; finished in {exec_time}");
            self.write_plain(time_str)?;
        }

        self.write_plain("\n\n")?;

        // Custom handling of cases where there is only 1 test to execute and that test was ignored.
        // We want to show more detailed information(why was the test ignored) for investigation purposes.
        if self.total_test_count == 1 && state.ignores.len() == 1 {
            let test_desc = &state.ignores[0].0;
            if let Some(im) = test_desc.ignore_message {
                self.write_plain(format!(
                    "test: {}, ignore_message: {}\n\n",
                    test_desc.name, im
                ))?;
            }
        }

        Ok(success)
    }
}

//######################################################################################################################
// From formatters/pretty.rs
// Source: https://github.com/rust-lang/rust/blob/104f4300cfddbd956e32820ef202a732f06ec848/library/test/src/formatters/pretty.rs#L13
// Changes: commented out any code that correspond to OutputLocation::Pretty and colors

pub(crate) struct PrettyFormatter<T> {
    out: OutputLocation<T>,
    //use_color: bool,
    time_options: Option<TestTimeOptions>,

    /// Number of columns to fill when aligning names
    max_name_len: usize,

    is_multithreaded: bool,
}

impl<T: Write> PrettyFormatter<T> {
    pub(crate) fn new(
        out: OutputLocation<T>,
        //use_color: bool,
        max_name_len: usize,
        is_multithreaded: bool,
        time_options: Option<TestTimeOptions>,
    ) -> Self {
        PrettyFormatter {
            out,
            //use_color,
            max_name_len,
            is_multithreaded,
            time_options,
        }
    }

    #[cfg(test)]
    pub(crate) fn output_location(&self) -> &OutputLocation<T> {
        &self.out
    }

    pub(crate) fn write_ok(&mut self) -> io::Result<()> {
        self.write_short_result("ok") //, term::color::GREEN)
    }

    pub(crate) fn write_failed(&mut self) -> io::Result<()> {
        self.write_short_result("FAILED") //, term::color::RED)
    }

    pub(crate) fn write_ignored(&mut self, message: Option<&'static str>) -> io::Result<()> {
        if let Some(message) = message {
            self.write_short_result(&format!("ignored, {message}")) //, term::color::YELLOW)
        } else {
            self.write_short_result("ignored") //, term::color::YELLOW)
        }
    }

    pub(crate) fn write_time_failed(&mut self) -> io::Result<()> {
        self.write_short_result("FAILED (time limit exceeded)") //, term::color::RED)
    }

    pub(crate) fn write_bench(&mut self) -> io::Result<()> {
        self.write_pretty("bench") //, term::color::CYAN)
    }

    pub(crate) fn write_short_result(
        &mut self,
        result: &str,
        //color: term::color::Color,
    ) -> io::Result<()> {
        self.write_pretty(result) //, color)
    }

    pub(crate) fn write_pretty(
        &mut self,
        word: &str,
        // color: term::color::Color
    ) -> io::Result<()> {
        match self.out {
            //OutputLocation::Pretty(ref mut term) => {
            //    if self.use_color {
            //        term.fg(color)?;
            //    }
            //    term.write_all(word.as_bytes())?;
            //    if self.use_color {
            //        term.reset()?;
            //    }
            //    term.flush()
            //}
            OutputLocation::Raw(ref mut stdout) => {
                stdout.write_all(word.as_bytes())?;
                stdout.flush()
            }
        }
    }

    pub(crate) fn write_plain<S: AsRef<str>>(&mut self, s: S) -> io::Result<()> {
        let s = s.as_ref();
        self.out.write_all(s.as_bytes())?;
        self.out.flush()
    }

    fn write_time(&mut self, desc: &TestDesc, exec_time: Option<&TestExecTime>) -> io::Result<()> {
        if let (Some(opts), Some(time)) = (self.time_options, exec_time) {
            let time_str = format!(" <{time}>");

            //let color = if self.use_color {
            //    if opts.is_critical(desc, time) {
            //        Some(term::color::RED)
            //    } else if opts.is_warn(desc, time) {
            //        Some(term::color::YELLOW)
            //    } else {
            //        None
            //    }
            //} else {
            //    None
            //};

            //match color {
            //    Some(color) => self.write_pretty(&time_str, color)?,
            //    None =>
            self.write_plain(&time_str)? //,
                                         //}
        }

        Ok(())
    }

    fn write_results(
        &mut self,
        inputs: &Vec<(TestDesc, Vec<u8>)>,
        results_type: &str,
    ) -> io::Result<()> {
        let results_out_str = format!("\n{results_type}:\n");

        self.write_plain(&results_out_str)?;

        let mut results = Vec::new();
        let mut stdouts = String::new();
        for (f, stdout) in inputs {
            results.push(f.name.as_slice().to_string()); // added call to as_slice() here
            if !stdout.is_empty() {
                stdouts.push_str(&format!("---- {} stdout ----\n", f.name));
                let output = String::from_utf8_lossy(stdout);
                stdouts.push_str(&output);
                stdouts.push('\n');
            }
        }
        if !stdouts.is_empty() {
            self.write_plain("\n")?;
            self.write_plain(&stdouts)?;
        }

        self.write_plain(&results_out_str)?;
        results.sort();
        for name in &results {
            self.write_plain(&format!("    {name}\n"))?;
        }
        Ok(())
    }

    pub(crate) fn write_successes(&mut self, state: &ConsoleTestState) -> io::Result<()> {
        self.write_results(&state.not_failures, "successes")
    }

    pub(crate) fn write_failures(&mut self, state: &ConsoleTestState) -> io::Result<()> {
        self.write_results(&state.failures, "failures")
    }

    pub(crate) fn write_time_failures(&mut self, state: &ConsoleTestState) -> io::Result<()> {
        self.write_results(&state.time_failures, "failures (time limit exceeded)")
    }

    fn write_test_name(&mut self, desc: &TestDesc) -> io::Result<()> {
        let name = desc.padded_name(self.max_name_len, desc.name.padding());
        if let Some(test_mode) = desc.test_mode() {
            self.write_plain(format!("test {name} - {test_mode} ... "))?;
        } else {
            self.write_plain(format!("test {name} ... "))?;
        }

        Ok(())
    }
}

impl<T: Write> OutputFormatter for PrettyFormatter<T> {
    fn write_run_start(&mut self, test_count: usize, shuffle_seed: Option<u64>) -> io::Result<()> {
        let noun = if test_count != 1 { "tests" } else { "test" };
        let shuffle_seed_msg = if let Some(shuffle_seed) = shuffle_seed {
            format!(" (shuffle seed: {shuffle_seed})")
        } else {
            String::new()
        };
        self.write_plain(format!("\nrunning {test_count} {noun}{shuffle_seed_msg}\n"))
    }

    fn write_test_start(&mut self, desc: &TestDesc) -> io::Result<()> {
        // When running tests concurrently, we should not print
        // the test's name as the result will be mis-aligned.
        // When running the tests serially, we print the name here so
        // that the user can see which test hangs.
        if !self.is_multithreaded {
            self.write_test_name(desc)?;
        }

        Ok(())
    }

    fn write_result(
        &mut self,
        desc: &TestDesc,
        result: &TestResult,
        exec_time: Option<&TestExecTime>,
        _: &[u8],
        _: &ConsoleTestState,
    ) -> io::Result<()> {
        if self.is_multithreaded {
            self.write_test_name(desc)?;
        }

        match *result {
            TestResult::TrOk => self.write_ok()?,
            TestResult::TrFailed | TestResult::TrFailedMsg(_) => self.write_failed()?,
            TestResult::TrIgnored => self.write_ignored(desc.ignore_message)?,
            //TestResult::TrBench(ref bs) => {
            //    self.write_bench()?;
            //    self.write_plain(format!(": {}", fmt_bench_samples(bs)))?;
            //}
            TestResult::TrTimedFail => self.write_time_failed()?,
        }

        self.write_time(desc, exec_time)?;
        self.write_plain("\n")
    }

    fn write_timeout(&mut self, desc: &TestDesc) -> io::Result<()> {
        self.write_plain(format!(
            "test {} has been running for over {} seconds\n",
            desc.name, TEST_WARN_TIMEOUT_S
        ))
    }

    fn write_run_finish(&mut self, state: &ConsoleTestState) -> io::Result<bool> {
        if state.options.display_output {
            self.write_successes(state)?;
        }
        let success = state.failed == 0;
        if !success {
            if !state.failures.is_empty() {
                self.write_failures(state)?;
            }

            if !state.time_failures.is_empty() {
                self.write_time_failures(state)?;
            }
        }

        self.write_plain("\ntest result: ")?;

        if success {
            // There's no parallelism at this point so it's safe to use color
            self.write_pretty("ok" /*, term::color::GREEN*/)?;
        } else {
            self.write_pretty("FAILED" /* , term::color::RED*/)?;
        }

        let s = format!(
            ". {} passed; {} failed; {} ignored; {} measured; {} filtered out",
            state.passed, state.failed, state.ignored, state.measured, state.filtered_out
        );

        self.write_plain(s)?;

        if let Some(ref exec_time) = state.exec_time {
            let time_str = format!("; finished in {exec_time}");
            self.write_plain(time_str)?;
        }

        self.write_plain("\n\n")?;

        Ok(success)
    }
}

//######################################################################################################################
// From formatters/json.rs
// Source: https://github.com/rust-lang/rust/blob/f37f8549940386a9d066ba199983affff47afbb4/library/test/src/formatters/mod.rs#L20
// Changes: added some calls to flush(), hoping to fix weird bug
// (first part of the json has been printed twice and obstructed one other line)

pub(crate) struct JsonFormatter<T> {
    out: OutputLocation<T>,
}

impl<T: Write> JsonFormatter<T> {
    pub(crate) fn new(out: OutputLocation<T>) -> Self {
        Self { out }
    }

    fn writeln_message(&mut self, s: &str) -> io::Result<()> {
        assert!(!s.contains('\n'));

        self.out.write_all(s.as_ref())?;
        self.out.flush()?;

        self.out.write_all(b"\n")?;
        self.out.flush()
    }

    fn write_message(&mut self, s: &str) -> io::Result<()> {
        assert!(!s.contains('\n'));

        self.out.write_all(s.as_ref())?;
        self.out.flush()
    }

    fn write_event(
        &mut self,
        ty: &str,
        name: &str,
        evt: &str,
        exec_time: Option<&TestExecTime>,
        stdout: Option<Cow<'_, str>>,
        extra: Option<&str>,
    ) -> io::Result<()> {
        // A doc test's name includes a filename which must be escaped for correct json.
        self.write_message(&*format!(
            r#"{{ "type": "{}", "name": "{}", "event": "{}""#,
            ty,
            EscapedString(name),
            evt
        ))?;
        if let Some(exec_time) = exec_time {
            self.write_message(&*format!(r#", "exec_time": {}"#, exec_time.0.as_secs_f64()))?;
        }
        if let Some(stdout) = stdout {
            self.write_message(&*format!(r#", "stdout": "{}""#, EscapedString(stdout)))?;
        }
        if let Some(extra) = extra {
            self.write_message(&*format!(r#", {}"#, extra))?;
        }
        self.writeln_message(" }")?;
        self.out.flush()
    }
}

impl<T: Write> OutputFormatter for JsonFormatter<T> {
    fn write_run_start(&mut self, test_count: usize, shuffle_seed: Option<u64>) -> io::Result<()> {
        let shuffle_seed_json = if let Some(shuffle_seed) = shuffle_seed {
            format!(r#", "shuffle_seed": {}"#, shuffle_seed)
        } else {
            String::new()
        };
        self.writeln_message(&*format!(
            r#"{{ "type": "suite", "event": "started", "test_count": {}{} }}"#,
            test_count, shuffle_seed_json
        ))
    }

    fn write_test_start(&mut self, desc: &TestDesc) -> io::Result<()> {
        self.writeln_message(&*format!(
            r#"{{ "type": "test", "event": "started", "name": "{}" }}"#,
            EscapedString(desc.name.as_slice())
        ))
    }

    fn write_result(
        &mut self,
        desc: &TestDesc,
        result: &TestResult,
        exec_time: Option<&TestExecTime>,
        stdout: &[u8],
        state: &ConsoleTestState,
    ) -> io::Result<()> {
        let display_stdout = state.options.display_output || *result != TestResult::TrOk;
        let stdout = if display_stdout && !stdout.is_empty() {
            Some(String::from_utf8_lossy(stdout))
        } else {
            None
        };
        match *result {
            TestResult::TrOk => {
                self.write_event("test", desc.name.as_slice(), "ok", exec_time, stdout, None)
            }

            TestResult::TrFailed => self.write_event(
                "test",
                desc.name.as_slice(),
                "failed",
                exec_time,
                stdout,
                None,
            ),

            TestResult::TrTimedFail => self.write_event(
                "test",
                desc.name.as_slice(),
                "failed",
                exec_time,
                stdout,
                Some(r#""reason": "time limit exceeded""#),
            ),

            TestResult::TrFailedMsg(ref m) => self.write_event(
                "test",
                desc.name.as_slice(),
                "failed",
                exec_time,
                stdout,
                Some(&*format!(r#""message": "{}""#, EscapedString(m))),
            ),

            TestResult::TrIgnored => self.write_event(
                "test",
                desc.name.as_slice(),
                "ignored",
                exec_time,
                stdout,
                desc.ignore_message
                    .map(|msg| format!(r#""message": "{}""#, EscapedString(msg)))
                    .as_deref(),
            ),
            //TestResult::TrBench(ref bs) => {
            //    let median = bs.ns_iter_summ.median as usize;
            //    let deviation = (bs.ns_iter_summ.max - bs.ns_iter_summ.min) as usize;
            //
            //    let mbps = if bs.mb_s == 0 {
            //        String::new()
            //    } else {
            //        format!(r#", "mib_per_second": {}"#, bs.mb_s)
            //    };
            //
            //    let line = format!(
            //        "{{ \"type\": \"bench\", \
            //         \"name\": \"{}\", \
            //         \"median\": {}, \
            //         \"deviation\": {}{} }}",
            //        EscapedString(desc.name.as_slice()),
            //        median,
            //        deviation,
            //        mbps
            //    );
            //
            //    self.writeln_message(&*line)
            //}
        }
    }

    fn write_timeout(&mut self, desc: &TestDesc) -> io::Result<()> {
        self.writeln_message(&*format!(
            r#"{{ "type": "test", "event": "timeout", "name": "{}" }}"#,
            EscapedString(desc.name.as_slice())
        ))
    }

    fn write_run_finish(&mut self, state: &ConsoleTestState) -> io::Result<bool> {
        self.write_message(&*format!(
            "{{ \"type\": \"suite\", \
             \"event\": \"{}\", \
             \"passed\": {}, \
             \"failed\": {}, \
             \"ignored\": {}, \
             \"measured\": {}, \
             \"filtered_out\": {}",
            if state.failed == 0 { "ok" } else { "failed" },
            state.passed,
            state.failed,
            state.ignored,
            state.measured,
            state.filtered_out,
        ))?;

        if let Some(ref exec_time) = state.exec_time {
            let time_str = format!(", \"exec_time\": {}", exec_time.0.as_secs_f64());
            self.write_message(&time_str)?;
        }

        self.writeln_message(" }")?;

        Ok(state.failed == 0)
    }
}

/// A formatting utility used to print strings with characters in need of escaping.
/// Base code taken form `libserialize::json::escape_str`
struct EscapedString<S: AsRef<str>>(S);

impl<S: AsRef<str>> std::fmt::Display for EscapedString<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        let mut start = 0;

        for (i, byte) in self.0.as_ref().bytes().enumerate() {
            let escaped = match byte {
                b'"' => "\\\"",
                b'\\' => "\\\\",
                b'\x00' => "\\u0000",
                b'\x01' => "\\u0001",
                b'\x02' => "\\u0002",
                b'\x03' => "\\u0003",
                b'\x04' => "\\u0004",
                b'\x05' => "\\u0005",
                b'\x06' => "\\u0006",
                b'\x07' => "\\u0007",
                b'\x08' => "\\b",
                b'\t' => "\\t",
                b'\n' => "\\n",
                b'\x0b' => "\\u000b",
                b'\x0c' => "\\f",
                b'\r' => "\\r",
                b'\x0e' => "\\u000e",
                b'\x0f' => "\\u000f",
                b'\x10' => "\\u0010",
                b'\x11' => "\\u0011",
                b'\x12' => "\\u0012",
                b'\x13' => "\\u0013",
                b'\x14' => "\\u0014",
                b'\x15' => "\\u0015",
                b'\x16' => "\\u0016",
                b'\x17' => "\\u0017",
                b'\x18' => "\\u0018",
                b'\x19' => "\\u0019",
                b'\x1a' => "\\u001a",
                b'\x1b' => "\\u001b",
                b'\x1c' => "\\u001c",
                b'\x1d' => "\\u001d",
                b'\x1e' => "\\u001e",
                b'\x1f' => "\\u001f",
                b'\x7f' => "\\u007f",
                _ => {
                    continue;
                }
            };

            if start < i {
                f.write_str(&self.0.as_ref()[start..i])?;
            }

            f.write_str(escaped)?;

            start = i + 1;
        }

        if start != self.0.as_ref().len() {
            f.write_str(&self.0.as_ref()[start..])?;
        }

        Ok(())
    }
}

//######################################################################################################################
// From formatters/mod.rs
// Source: https://github.com/rust-lang/rust/blob/f37f8549940386a9d066ba199983affff47afbb4/library/test/src/formatters/mod.rs#L20

pub(crate) trait OutputFormatter {
    fn write_run_start(&mut self, test_count: usize, shuffle_seed: Option<u64>) -> io::Result<()>;
    fn write_test_start(&mut self, desc: &TestDesc) -> io::Result<()>;
    fn write_timeout(&mut self, desc: &TestDesc) -> io::Result<()>;
    fn write_result(
        &mut self,
        desc: &TestDesc,
        result: &TestResult,
        exec_time: Option<&TestExecTime>,
        stdout: &[u8],
        state: &ConsoleTestState,
    ) -> io::Result<()>;
    fn write_run_finish(&mut self, state: &ConsoleTestState) -> io::Result<bool>;
}

pub(crate) fn write_stderr_delimiter(test_output: &mut Vec<u8>, test_name: &TestName) {
    match test_output.last() {
        Some(b'\n') => (),
        Some(_) => test_output.push(b'\n'),
        None => (),
    }
    writeln!(test_output, "---- {} stderr ----", test_name).unwrap();
}

//######################################################################################################################
// From term/terminfo/mod.rs
// Source:

/// A parsed terminfo database entry.
#[allow(unused)]
#[derive(Debug)]
pub(crate) struct TermInfo {
    /// Names for the terminal
    pub(crate) names: Vec<String>,
    /// Map of capability name to boolean value
    pub(crate) bools: HashMap<String, bool>,
    /// Map of capability name to numeric value
    pub(crate) numbers: HashMap<String, u32>,
    /// Map of capability name to raw (unexpanded) string
    pub(crate) strings: HashMap<String, Vec<u8>>,
}

/// A terminfo creation error.
#[derive(Debug)]
pub(crate) enum Error {
    /// TermUnset Indicates that the environment doesn't include enough information to find
    /// the terminfo entry.
    TermUnset,
    /// MalformedTerminfo indicates that parsing the terminfo entry failed.
    MalformedTerminfo(String),
    /// io::Error forwards any io::Errors encountered when finding or reading the terminfo entry.
    IoError(io::Error),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        use Error::*;
        match self {
            IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Error::*;
        match *self {
            TermUnset => Ok(()),
            MalformedTerminfo(ref e) => e.fmt(f),
            IoError(ref e) => e.fmt(f),
        }
    }
}

//######################################################################################################################
// From console.rs
// Source: https://github.com/rust-lang/rust/blob/f37f8549940386a9d066ba199983affff47afbb4/library/test/src/console.rs#L44
// Changes: Commented out enum variant Pretty, adapted visibility

/// Generic wrapper over stdout.
pub(crate) enum OutputLocation<T> {
    //    Pretty(Box<term::StdoutTerminal>),
    Raw(T),
}

impl<T: Write> Write for OutputLocation<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self {
            //OutputLocation::Pretty(ref mut term) => term.write(buf),
            OutputLocation::Raw(ref mut stdout) => stdout.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self {
            //OutputLocation::Pretty(ref mut term) => term.flush(),
            OutputLocation::Raw(ref mut stdout) => stdout.flush(),
        }
    }
}

pub(crate) struct ConsoleTestState {
    pub(crate) log_out: Option<File>,
    pub(crate) total: usize,
    pub(crate) passed: usize,
    pub(crate) failed: usize,
    pub(crate) ignored: usize,
    pub(crate) filtered_out: usize,
    pub(crate) measured: usize,
    pub(crate) exec_time: Option<TestSuiteExecTime>,
    pub(crate) metrics: MetricMap,
    pub(crate) failures: Vec<(TestDesc, Vec<u8>)>,
    pub(crate) not_failures: Vec<(TestDesc, Vec<u8>)>,
    pub ignores: Vec<(TestDesc, Vec<u8>)>,
    pub(crate) time_failures: Vec<(TestDesc, Vec<u8>)>,
    pub(crate) options: Options,
}

impl ConsoleTestState {
    pub(crate) fn new(opts: &TestOpts) -> io::Result<ConsoleTestState> {
        let log_out = match opts.logfile {
            Some(ref path) => Some(File::create(path)?),
            None => None,
        };

        Ok(ConsoleTestState {
            log_out,
            total: 0,
            passed: 0,
            failed: 0,
            ignored: 0,
            filtered_out: 0,
            measured: 0,
            exec_time: None,
            metrics: MetricMap::new(),
            failures: Vec::new(),
            not_failures: Vec::new(),
            ignores: Vec::new(),
            time_failures: Vec::new(),
            options: opts.options,
        })
    }

    pub(crate) fn write_log<F, S>(&mut self, msg: F) -> io::Result<()>
    where
        S: AsRef<str>,
        F: FnOnce() -> S,
    {
        match self.log_out {
            None => Ok(()),
            Some(ref mut o) => {
                let msg = msg();
                let msg = msg.as_ref();
                o.write_all(msg.as_bytes())
            }
        }
    }

    pub(crate) fn write_log_result(
        &mut self,
        test: &TestDesc,
        result: &TestResult,
        exec_time: Option<&TestExecTime>,
    ) -> io::Result<()> {
        self.write_log(|| {
            let TestDesc {
                name,
                ignore_message,
                ..
            } = test;
            format!(
                "{} {}",
                match *result {
                    TestResult::TrOk => "ok".to_owned(),
                    TestResult::TrFailed => "failed".to_owned(),
                    TestResult::TrFailedMsg(ref msg) => format!("failed: {msg}"),
                    TestResult::TrIgnored => {
                        if let Some(msg) = ignore_message {
                            format!("ignored: {msg}")
                        } else {
                            "ignored".to_owned()
                        }
                    }
                    //TestResult::TrBench(ref bs) => fmt_bench_samples(bs),
                    TestResult::TrTimedFail => "failed (time limit exceeded)".to_owned(),
                },
                name,
            )
        })?;
        if let Some(exec_time) = exec_time {
            self.write_log(|| format!(" <{exec_time}>"))?;
        }
        self.write_log(|| "\n")
    }

    fn current_test_count(&self) -> usize {
        self.passed + self.failed + self.ignored + self.measured
    }
}

// Calculates padding for given test description.
pub(crate) fn len_if_padded(t: &TestDescAndFn) -> usize {
    match t.testfn.padding() {
        NamePadding::PadNone => 0,
        NamePadding::PadOnRight => t.desc.name.as_slice().len(),
    }
}

//######################################################################################################################
// From time.rs
// Source: https://github.com/rust-lang/rust/blob/f37f8549940386a9d066ba199983affff47afbb4/library/test/src/time.rs#L75

/// The measured execution time of the whole test suite.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TestSuiteExecTime(pub(crate) Duration);

impl fmt::Display for TestSuiteExecTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}s", self.0.as_secs_f64())
    }
}

//######################################################################################################################
// From test_result.rs
// Source: https://github.com/rust-lang/rust/blob/104f4300cfddbd956e32820ef202a732f06ec848/library/test/src/test_result.rs#L16
// Changes: added Serialize and Deserialize to derive macro and commented out TrBench

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(crate) enum TestResult {
    TrOk,
    TrFailed,
    TrFailedMsg(String),
    TrIgnored,
    // TrBench(BenchSamples),
    TrTimedFail,
}

/// Creates a `TestResult` depending on the raw result of test execution
/// and associated data.
pub(crate) fn calc_result<'a>(
    desc: &TestDesc,
    task_result: Result<(), &'a (dyn Any + 'static + Send)>,
    time_opts: &Option<TestTimeOptions>,
    exec_time: &Option<TestExecTime>,
) -> TestResult {
    let result = match (&desc.should_panic, task_result) {
        (&ShouldPanic::No, Ok(())) | (&ShouldPanic::Yes, Err(_)) => TestResult::TrOk,
        (&ShouldPanic::YesWithMessage(msg), Err(ref err)) => {
            let maybe_panic_str = err
                .downcast_ref::<String>()
                .map(|e| &**e)
                .or_else(|| err.downcast_ref::<&'static str>().copied());

            if maybe_panic_str.map(|e| e.contains(msg)).unwrap_or(false) {
                TestResult::TrOk
            } else if let Some(panic_str) = maybe_panic_str {
                TestResult::TrFailedMsg(format!(
                    r#"panic did not contain expected string
      panic message: `{:?}`,
 expected substring: `{:?}`"#,
                    panic_str, msg
                ))
            } else {
                TestResult::TrFailedMsg(format!(
                    r#"expected panic with string value,
 found non-string value: `{:?}`
     expected substring: `{:?}`"#,
                    (**err).type_id(),
                    msg
                ))
            }
        }
        (&ShouldPanic::Yes, Ok(())) | (&ShouldPanic::YesWithMessage(_), Ok(())) => {
            TestResult::TrFailedMsg("test did not panic as expected".to_string())
        }
        _ => TestResult::TrFailed,
    };

    // If test is already failed (or allowed to fail), do not change the result.
    if result != TestResult::TrOk {
        return result;
    }

    // Check if test is failed due to timeout.
    if let (Some(opts), Some(time)) = (time_opts, exec_time) {
        if opts.error_on_excess && opts.is_critical(desc, time) {
            return TestResult::TrTimedFail;
        }
    }

    result
}
