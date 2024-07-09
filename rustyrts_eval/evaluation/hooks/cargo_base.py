import gc
import glob
import os
import re
from abc import ABC, abstractmethod
from pathlib import Path
from time import time
from typing import Optional
import tempfile

from ..hooks.base import Hook
from ...db.base import DBConnection
from ...db.history import DBTestReport
from ...models.scm.git import GitClient
from ...models.scm.base import Repository, Commit
from ...models.testing.base import TestReport
from ...models.testing.loaders.cargo_test import CargoTestTestReportLoader
from ...util.os.exec import SubprocessContainer
from ...util.logging.logger import get_logger
from os.path import abspath

_LOGGER = get_logger(__name__)


def env_tmp_override():
    return {"TRYBUILD": "overwrite", "INSTA_UPDATE": "always"}


class CargoHook(Hook, ABC):
    def __init__(
        self,
        repository: Repository,
        git_client: GitClient,
        connection: DBConnection,
        env_vars: Optional[dict[str, str]] = None,
        build_options=None,
        test_options=None,
        report_name: Optional[str] = None,
        output_path: Optional[str] = None,
    ):
        super().__init__(repository, output_path, git_client)
        if self.output_path:
            self.cache_dir = os.path.join(self.output_path, ".cargo-hook")
        else:
            self.cache_dir = os.path.join(tempfile.gettempdir(), ".cargo-hook")
        self.report_name = report_name
        self.connection = connection
        self.env_vars = env_vars if env_vars else {}
        self.target_dir = abspath(repository.path + "/target_test")
        self.build_options = build_options if build_options else []
        self.test_options = test_options if test_options else []

        # Check if we need to build before testing
        self.build_debug = False
        self.build_release = False
        for file in glob.glob("**/*.rs", recursive=True):
            if os.path.isfile(file):
                content = open(file, "r").read()
                if "target/debug" in content:
                    self.build_debug = True
                if "target/debug" in content:
                    self.build_release = True

    def env(self) -> dict[str, str]:
        os.makedirs(self.target_dir, exist_ok=True)
        env = {"CARGO_TARGET_DIR": self.target_dir}
        return os.environ | self.env_vars | env

    def build_env(self) -> dict[str, str]:
        return os.environ | self.env_vars

    def clean_command(self):
        return "cargo clean"

    def update_command(self):
        return "cargo update"

    def build_command(self, features) -> str:
        build_options = " ".join(self.build_options) + (" --features {0}".format(features) if features else "")
        return "cargo build {0}".format(build_options)

    @abstractmethod
    def test_command_parent(self, features) -> str:
        pass

    @abstractmethod
    def test_command(self, features) -> str:
        pass

    def run(
        self,
        commit: Commit,
        features_parent: Optional[str],
        features: Optional[str],
    ) -> bool:
        """
        Run cargo test.

        :return:
        """
        has_errored = False

        with self.connection.create_session_ctx() as session:
            tmp_path = os.getcwd()

            os.makedirs(self.cache_dir, exist_ok=True)  # recursively create dirs

            # navigate into repo
            os.chdir(self.repository.path)

            ############################################################################################################
            # Prepare on parent commit

            # checkout parent commit
            parent_commit = self.git_client.get_parent_commit(commit_sha=commit.commit_str)
            self.git_client.git_repo.git.checkout(parent_commit, force=True)
            self.git_client.git_repo.git.reset(parent_commit, hard=True)

            for submodule in self.git_client.git_repo.submodules:
                submodule.update(init=True, recursive=True, force=True)

            for filename in glob.glob("rust-toolchain*"):
                os.remove(filename)
            if os.path.exists(".cargo"):
                os.remove(".cargo")  # Cargo config overwrites the one we are setting in .cargo/config.toml

            # clean
            proc: SubprocessContainer = SubprocessContainer(
                command=self.clean_command(),
                output_filepath=self.prepare_cache_file(),
                env=self.env(),
            )
            proc.execute(capture_output=True, shell=True, timeout=100.0)

            # update dependencies
            self.update_dependencies(parent_commit)

            if self.build_debug:
                ########################################################################################################
                # Build debug on parent commit
                # (Some tests in certain projects require artifacts that are build only by cargo build)

                if not has_errored:
                    # Run build command on parent commit
                    proc: SubprocessContainer = SubprocessContainer(
                        command=self.build_command(features_parent),
                        output_filepath=self.prepare_cache_file(),
                        env=self.build_env(),
                    )
                    proc.execute(capture_output=True, shell=True, timeout=10000.0)

                    # Do not consider it an error if the build command fails
                    # has_errored |= not (proc.exit_code == 0 or any(
                    #    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                    # ******************************************************************************************************
                    # Parse result

                    log = proc.output

                    # create test report object
                    test_report: TestReport = TestReport(
                        name=self.report_name + " - parent build debug",
                        duration=proc.end_to_end_time,
                        build_duration=(CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None),
                        suites=[],
                        commit=commit,
                        commit_str=commit.commit_str,
                        log=log,
                        has_failed=proc.exit_code != 0,
                        has_errored=has_errored,
                    )

                    DBTestReport.create_or_update(report=test_report, session=session)

            if self.build_release:
                ########################################################################################################
                # Build release on parent commit
                # (Some tests in certain projects require artifacts that are build only by cargo build)

                if not has_errored:
                    # Run build command on parent commit
                    proc: SubprocessContainer = SubprocessContainer(
                        command=self.build_command(features_parent) + " --release",
                        output_filepath=self.prepare_cache_file(),
                        env=self.build_env(),
                    )
                    proc.execute(capture_output=True, shell=True, timeout=10000.0)

                    # Do not consider it an error if the build command fails
                    # has_errored |= not (proc.exit_code == 0 or any(
                    #    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                    # ******************************************************************************************************
                    # Parse result

                    log = proc.output

                    # create test report object
                    test_report: TestReport = TestReport(
                        name=self.report_name + " - parent build release",
                        duration=proc.end_to_end_time,
                        build_duration=(CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None),
                        suites=[],
                        commit=commit,
                        commit_str=commit.commit_str,
                        log=log,
                        has_failed=proc.exit_code != 0,
                        has_errored=has_errored,
                    )

                    DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            # Run on parent commit
            if not has_errored:
                # Run test command to generate temporary files for incremental compilation or traces
                proc: SubprocessContainer = SubprocessContainer(
                    command=self.test_command_parent(features_parent),
                    output_filepath=self.prepare_cache_file(),
                    env=self.env() | env_tmp_override(),
                )
                proc.execute(capture_output=True, shell=True, timeout=10000.0)
                has_errored |= proc.exit_code == -1 or "thread caused non-unwinding panic. aborting." in proc.output or (not (proc.exit_code == 0 or any(line.startswith("{") and line.endswith("}") for line in proc.output.splitlines())))

                # ******************************************************************************************************
                # Parse result

                log = proc.output

                # parse test_suites
                loader = CargoTestTestReportLoader(proc.output, load_ignored=False)
                try:
                    test_suites = loader.load()
                except:
                    has_errored = True
                    log = "Failed to parse testsuites\n" + log
                    test_suites = []

                # create test report object
                test_report: TestReport = TestReport(
                    name=self.report_name + " - parent",
                    duration=proc.end_to_end_time,
                    build_duration=(CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None),
                    suites=test_suites,
                    commit=commit,
                    commit_str=commit.commit_str,
                    log=log,
                    has_failed=proc.exit_code != 0,
                    has_errored=has_errored,
                )

                DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            # Prepare on actual commit

            # checkout actual commit
            self.git_client.git_repo.git.checkout(commit.commit_str, force=True)
            self.git_client.git_repo.git.reset(commit.commit_str, hard=True)

            for submodule in self.git_client.git_repo.submodules:
                submodule.update(init=True, recursive=True, force=True)

            for filename in glob.glob("rust-toolchain*"):
                os.remove(filename)
            if os.path.exists(".cargo"):
                os.remove(".cargo")  # Cargo config overwrites the one we are setting in .cargo/config.toml

            # update dependencies
            self.update_dependencies(commit)

            if self.build_debug:
                ########################################################################################################
                # Build debug on actual commit
                # (Some tests in certain projects require artifacts that are build only by cargo build)

                if not has_errored:
                    # Run build command on actual commit
                    proc: SubprocessContainer = SubprocessContainer(
                        command=self.build_command(features),
                        output_filepath=self.prepare_cache_file(),
                        env=self.build_env(),
                    )
                    proc.execute(capture_output=True, shell=True, timeout=10000.0)

                    # Do not consider it an error if the build command fails
                    # has_errored |= not (proc.exit_code == 0 or any(
                    #    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                    # ******************************************************************************************************
                    # Parse result

                    log = proc.output

                    # create test report object
                    test_report: TestReport = TestReport(
                        name=self.report_name + " - build debug",
                        duration=proc.end_to_end_time,
                        build_duration=(CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None),
                        suites=[],
                        commit=commit,
                        commit_str=commit.commit_str,
                        log=log,
                        has_failed=proc.exit_code != 0,
                        has_errored=has_errored,
                    )

                    DBTestReport.create_or_update(report=test_report, session=session)

            if self.build_release:
                ########################################################################################################
                # Build release on actual commit
                # (Some tests in certain projects require artifacts that are build only by cargo build)

                if not has_errored:
                    # Run build command on actual commit
                    proc: SubprocessContainer = SubprocessContainer(
                        command=self.build_command(features) + " --release",
                        output_filepath=self.prepare_cache_file(),
                        env=self.build_env(),
                    )
                    proc.execute(capture_output=True, shell=True, timeout=10000.0)

                    # Do not consider it an error if the build command fails
                    # has_errored |= not (proc.exit_code == 0 or any(
                    #    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                    # ******************************************************************************************************
                    # Parse result

                    log = proc.output

                    # create test report object
                    test_report: TestReport = TestReport(
                        name=self.report_name + " - build release",
                        duration=proc.end_to_end_time,
                        build_duration=(CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None),
                        suites=[],
                        commit=commit,
                        commit_str=commit.commit_str,
                        log=log,
                        has_failed=proc.exit_code != 0,
                        has_errored=has_errored,
                    )

                    DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            # Run on actual commit

            if not has_errored:
                # Run test command on actual commit
                proc: SubprocessContainer = SubprocessContainer(
                    command=self.test_command(features),
                    output_filepath=self.prepare_cache_file(),
                    env=self.env()
                    # | {"CARGO_LOG": "debug"}
                    # e.g. changes trybuild files will be overridden by git reset, so we just use it here as well
                    # this effectively prevents those tests from failing, but there is just no other way
                    | env_tmp_override(),
                )
                proc.execute(capture_output=True, shell=True, timeout=10000.0)
                has_errored |= proc.exit_code == -1 or "thread caused non-unwinding panic. aborting." in proc.output or (not (proc.exit_code == 0 or any(line.startswith("{") and line.endswith("}") for line in proc.output.splitlines())))

                # ******************************************************************************************************
                # Parse result

                log = proc.output

                # parse test_suites
                loader = CargoTestTestReportLoader(proc.output, load_ignored=False)
                try:
                    test_suites = loader.load()
                except:
                    has_errored = True
                    log = "Failed to parse testsuites\n" + log
                    test_suites = []

                # create test report object
                test_report: TestReport = TestReport(
                    name=self.report_name,
                    duration=proc.end_to_end_time,
                    build_duration=(CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None),
                    suites=test_suites,
                    commit=commit,
                    commit_str=commit.commit_str,
                    log=log,
                    has_failed=proc.exit_code != 0,
                    has_errored=has_errored,
                )

                DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            session.commit()

            # return to previous directory
            os.chdir(tmp_path)

        self.git_client.git_repo.git.reset(commit.commit_str, hard=True)
        self.git_client.clean(rm_dirs=True)

        freed = gc.collect()
        _LOGGER.info("gc has freed " + str(freed) + " objects")

        return not has_errored

    def update_dependencies(self, commit):
        if not self.git_client.get_file_is_tracked(commit, "Cargo.lock"):  # if Cargo.lock is versioned using git, we do not want to update all packages
            update_command = self.update_command()
            proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
            proc.execute(capture_output=True, shell=True, timeout=100.0)
        else:
            _LOGGER.debug("Found versioned Carg.lock, skipping update of all dependencies")

        ## The following commands will just silently fail if not applicable

        # additionally update actix_derive which has shown to be problematic
        update_command = self.update_command() + " actix_derive --precise 0.6.0"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update chrono which has shown to be problematic
        update_command = self.update_command() + " chrono --precise 0.4.29"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update regex which has shown to be problematic
        update_command = self.update_command() + " regex --precise 1.4.3"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update proc-macro2@1 which has shown to be problematic in several projects
        update_command = self.update_command() + " proc-macro2@1"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update reedline which has shown to be problematic in meilisearch
        # update_command = self.update_command() + " reedline"
        # proc: SubprocessContainer = SubprocessContainer(
        #     command=update_command, output_filepath=self.prepare_cache_file()
        # )
        # proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update value-bag which has shown to be problematic in feroxbuster
        update_command = self.update_command() + " value-bag"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update log which has shown to be problematic in feroxbuster
        update_command = self.update_command() + " log"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update rustc-serialize which has shown to be problematic in zenoh
        update_command = self.update_command() + " rustc-serialize"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update log@0.4.14 which have shown to be problematic in zenoh
        versions = ["0.4.14"]
        for v in versions:
            update_command = self.update_command() + " log@" + v
            proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
            proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update tokio which have shown to be problematic in penumbra
        update_command = self.update_command() + " tokio"
        proc: SubprocessContainer = SubprocessContainer(command=update_command, output_filepath=self.prepare_cache_file())
        proc.execute(capture_output=True, shell=True, timeout=100.0)

    def prepare_cache_file(self) -> str:
        # prepare cache dir/file
        cache_file = "run_{}.log".format(int(time() * 1000))  # run identified by timestamp
        cache_file_path = os.path.join(self.cache_dir, cache_file)
        return cache_file_path
