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


class CargoHook(Hook, ABC):

    def __init__(
            self,
            repository: Repository,
            git_client: GitClient,
            connection: DBConnection,
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

    @abstractmethod
    def env(self):
        pass

    @abstractmethod
    def clean_command(self):
        pass

    @abstractmethod
    def build_command(self):
        pass

    @abstractmethod
    def test_command(self):
        pass

    def run(self, commit: Commit) -> bool:
        """
        Run cargo test.

        :return:
        """
        # keep track of current working directory
        has_failed = False

        with self.connection.create_session_ctx() as session:
            tmp_path = os.getcwd()

            os.makedirs(self.cache_dir, exist_ok=True)  # recursively create dirs

            # navigate into repo
            os.chdir(self.repository.path)

            ############################################################################################################
            # Prepare on parent commit
            if not has_failed:
                # checkout parent commit
                parent_commit = self.git_client.get_parent_commit(commit_sha=commit.commit_str)
                self.git_client.git_repo.git.checkout(parent_commit, force=True)
                self.git_client.git_repo.git.reset(parent_commit, hard=True)

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

                # prepare cache dir/file
                cache_file = "run_{}.log".format(
                    int(time() * 1000)
                )  # run identified by timestamp
                cache_file_path = os.path.join(self.cache_dir, cache_file)

                # Run build command to generate temporary files for incremental compilation
                proc: SubprocessContainer = SubprocessContainer(
                    command=self.build_command(), output_filepath=cache_file_path, env=self.env()
                )
                proc.execute(capture_output=True, shell=True, timeout=10000.0)
                has_failed |= not (proc.exit_code == 0 or any(
                    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                # ******************************************************************************************************
                # Parse result

                # parse test_suites
                loader = CargoTestTestReportLoader(proc.output, load_ignored=False)
                test_suites = loader.load()

                # create test report object
                test_report: TestReport = TestReport(
                    name=self.report_name + " - parent",
                    duration=proc.end_to_end_time,
                    suites=test_suites,
                    commit=commit,
                    commit_str=commit.commit_str,
                    log=proc.output,
                    has_failed=has_failed
                )

                DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            # Run on actual commit

            if not has_failed:
                # checkout actual commit
                self.git_client.git_repo.git.checkout(commit.commit_str, force=True)
                self.git_client.git_repo.git.reset(commit.commit_str, hard=True)

                for filename in glob.glob("rust-toolchain*"):
                    os.remove(filename)

                # prepare cache dir/file
                cache_file = "run_{}.log".format(
                    int(time() * 1000)
                )  # run identified by timestamp
                cache_file_path = os.path.join(self.cache_dir, cache_file)

                # Run test command on actual commit
                proc: SubprocessContainer = SubprocessContainer(
                    command=self.test_command(), output_filepath=cache_file_path, env=self.env()
                )
                proc.execute(capture_output=True, shell=True, timeout=10000.0)
                has_failed |= not (proc.exit_code == 0 or any(
                    line.startswith("{") and line.endswith("}") for line in proc.output.splitlines()))

                # ******************************************************************************************************
                # Parse result

                # parse test_suites
                loader = CargoTestTestReportLoader(proc.output, load_ignored=False)
                test_suites = loader.load()

                # create test report object
                test_report: TestReport = TestReport(
                    name=self.report_name,
                    duration=proc.end_to_end_time,
                    suites=test_suites,
                    commit=commit,
                    commit_str=commit.commit_str,
                    log=proc.output,
                    has_failed=has_failed
                )

                DBTestReport.create_or_update(report=test_report, session=session)

            ############################################################################################################
            session.commit()

            # return to previous directory
            os.chdir(tmp_path)

        return not has_failed
