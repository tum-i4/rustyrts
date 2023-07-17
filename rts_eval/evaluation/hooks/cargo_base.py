import gc
import glob
import os
from abc import ABC, abstractmethod
from time import time
from typing import Optional
import tempfile

from ..base import Hook
from ...db.base import DBConnection
from ...db.testing import DBTestReport
from ...models.scm.git import GitClient
from ...models.scm.base import Repository, Commit
from ...models.testing.base import TestReport
from ...models.testing.loaders.cargo_test import CargoTestTestReportLoader
from ...util.os.exec import SubprocessContainer
from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)


def env_tmp_override():
    return {"TRYBUILD": "overwrite", "INSTA_UPDATE": "always"}


class CargoHook(Hook, ABC):

    def __init__(
            self,
            repository: Repository,
            git_client: GitClient,
            connection: DBConnection,
            report_name: Optional[str] = None,
            output_path: Optional[str] = None,
            build=False
    ):
        super().__init__(repository, output_path, git_client)
        if self.output_path:
            self.cache_dir = os.path.join(self.output_path, ".cargo-hook")
        else:
            self.cache_dir = os.path.join(tempfile.gettempdir(), ".cargo-hook")
        self.report_name = report_name
        self.connection = connection
        self.build = build

    @abstractmethod
    def env(self):
        pass

    @abstractmethod
    def build_env(self):
        pass

    @abstractmethod
    def clean_command(self):
        pass

    @abstractmethod
    def build_command(self, features):
        pass

    @abstractmethod
    def test_command_parent(self, features):
        pass

    @abstractmethod
    def test_command(self, features):
        pass

    def run(self, commit: Commit, features_parent: Optional[str], features: Optional[str]) -> bool:
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
                try:
                    submodule.remove(module=True, force=True, configuration=True)
                    submodule.update(init=True, recursive=True, force=True)
                except:
                    continue

            for filename in glob.glob("rust-toolchain*"):
                os.remove(filename)

            # prepare cache dir/file
            cache_file = "run_{}.log".format(
                int(time() * 1000)
            )  # run identified by timestamp
            cache_file_path = os.path.join(self.cache_dir, cache_file)

            # clean
            proc: SubprocessContainer = SubprocessContainer(
                command=self.clean_command(), output_filepath=cache_file_path
            )
            proc.execute(capture_output=True, shell=True, timeout=100.0)

            # Check if we need to build before testing
            for file in glob.glob("**/*.rs", recursive=True):
                if os.path.isfile(file):
                    content = open(file, "r").read()
                    if "target/debug" in content:
                        self.build = True
                        break

            if self.build:
                ########################################################################################################
                # Build on parent commit
                # (Some tests in certain projects require artifacts that are build only by cargo build)

                if not has_errored:
                    # prepare cache dir/file
                    cache_file = "run_{}.log".format(
                        int(time() * 1000)
                    )  # run identified by timestamp
                    cache_file_path = os.path.join(self.cache_dir, cache_file)

                    # Run build command on parent commit
                    proc: SubprocessContainer = SubprocessContainer(
                        command=self.build_command(features_parent), output_filepath=cache_file_path,
                        env=self.build_env()
                    )
                    proc.execute(capture_output=True, shell=True, timeout=10000.0)

                    has_errored |= not (proc.exit_code == 0 or any(
                        line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                    # ******************************************************************************************************
                    # Parse result

                    log = proc.output

                    # create test report object
                    test_report: TestReport = TestReport(
                        name=self.report_name + " - parent build",
                        duration=proc.end_to_end_time,
                        build_duration=CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None,
                        suites=[],
                        commit=commit,
                        commit_str=commit.commit_str,
                        log=log,
                        has_failed=proc.exit_code != 0,
                        has_errored=has_errored
                    )

                    DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            # Run on parent commit
            if not has_errored:
                # prepare cache dir/file
                cache_file = "run_{}.log".format(
                    int(time() * 1000)
                )  # run identified by timestamp
                cache_file_path = os.path.join(self.cache_dir, cache_file)

                # Run test command to generate temporary files for incremental compilation or traces
                proc: SubprocessContainer = SubprocessContainer(
                    command=self.test_command_parent(features_parent),
                    output_filepath=cache_file_path,
                    env=self.env() | env_tmp_override()
                )
                proc.execute(capture_output=True, shell=True, timeout=10000.0)
                has_errored |= not (proc.exit_code == 0 or any(
                    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

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
                    build_duration=CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None,
                    suites=test_suites,
                    commit=commit,
                    commit_str=commit.commit_str,
                    log=log,
                    has_failed=proc.exit_code != 0,
                    has_errored=has_errored
                )

                DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            # Prepare on actual commit

            # checkout actual commit
            self.git_client.git_repo.git.checkout(commit.commit_str, force=True)
            self.git_client.git_repo.git.reset(commit.commit_str, hard=True)

            for submodule in self.git_client.git_repo.submodules:
                try:
                    submodule.remove(module=True, force=True, configuration=True)
                    submodule.update(init=True, recursive=True, force=True)
                except:
                    continue

            for filename in glob.glob("rust-toolchain*"):
                os.remove(filename)

            if self.build:
                ########################################################################################################
                # Build on actual commit
                # (Some tests in certain projects require artifacts that are build only by cargo build)

                if not has_errored:
                    # prepare cache dir/file
                    cache_file = "run_{}.log".format(
                        int(time() * 1000)
                    )  # run identified by timestamp
                    cache_file_path = os.path.join(self.cache_dir, cache_file)

                    # Run build command on actual commit
                    proc: SubprocessContainer = SubprocessContainer(
                        command=self.build_command(features), output_filepath=cache_file_path, env=self.build_env()
                    )
                    proc.execute(capture_output=True, shell=True, timeout=10000.0)
                    has_errored |= not (proc.exit_code == 0 or any(
                        line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                    # ******************************************************************************************************
                    # Parse result

                    log = proc.output

                    # create test report object
                    test_report: TestReport = TestReport(
                        name=self.report_name + " - build",
                        duration=proc.end_to_end_time,
                        build_duration=CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None,
                        suites=[],
                        commit=commit,
                        commit_str=commit.commit_str,
                        log=log,
                        has_failed=proc.exit_code != 0,
                        has_errored=has_errored
                    )

                    DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            # Run on actual commit

            if not has_errored:
                # prepare cache dir/file
                cache_file = "run_{}.log".format(
                    int(time() * 1000)
                )  # run identified by timestamp
                cache_file_path = os.path.join(self.cache_dir, cache_file)

                # Run test command on actual commit
                proc: SubprocessContainer = SubprocessContainer(
                    command=self.test_command(features), output_filepath=cache_file_path,
                    env=self.env() | env_tmp_override() | {"RUSTYRTS_LOG": "debug"}
                    # e.g. changes trybuild files will be overridden by git reset, so we just use it here as well
                    # this effectively prevents those tests from failing, but there is just no other way
                )
                proc.execute(capture_output=True, shell=True, timeout=10000.0)
                has_errored |= not (proc.exit_code == 0 or any(
                    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

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
                    build_duration=CargoTestTestReportLoader.parse_build_time(log) if not has_errored else None,
                    suites=test_suites,
                    commit=commit,
                    commit_str=commit.commit_str,
                    log=log,
                    has_failed=proc.exit_code != 0,
                    has_errored=has_errored
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
