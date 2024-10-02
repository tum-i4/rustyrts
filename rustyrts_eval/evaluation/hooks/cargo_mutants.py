import gc
import os
import re
from enum import Enum
from time import time
from typing import Optional, Dict, Callable
import tempfile

from ..hooks.base import Hook
from ...db.base import DBConnection
from ...db.mutants import DBMutantsReport
from ...models.scm.git import GitClient
from ...models.scm.base import Repository, Commit
from ...models.testing.loaders.mutants import CargoMutantsTestReportLoader
from ...models.testing.mutants import MutantsReport
from ...util.os.exec import SubprocessContainer
from ...util.logging.logger import get_logger

_LOGGER = get_logger(__name__)
ask_for_skip = True


class RustyRTSMode(str, Enum):
    TEST = ""
    BASIC = " basic"
    DYNAMIC = " dynamic"
    STATIC = " static"


class CargoMutantsHook(Hook):
    def __init__(
        self,
        repository: Repository,
        git_client: GitClient,
        mode: RustyRTSMode,
        connection: DBConnection,
        env_vars: Optional[Dict] = None,
        options=None,
        test_options=None,
        output_path: Optional[str] = None,
        pre_hook: Optional[Callable] = None,
    ):
        super().__init__(repository, output_path, git_client)
        if self.output_path:
            self.cache_dir = os.path.join(self.output_path, ".cargo-hook")
        else:
            self.cache_dir = os.path.join(tempfile.gettempdir(), ".cargo-hook")
        self.mode = mode
        self.env_vars = env_vars
        self.options = options if options else ""
        self.test_options = test_options if test_options else ""
        self.connection = connection
        self.pre_hook = pre_hook

    def mutants_command(self, individual_mutants_options, individual_test_options):
        mutants_options = " ".join([self.options, individual_mutants_options])
        test_options = " ".join([self.test_options, individual_test_options])
        return "cargo mutants-rts{0} {1} -- {2}".format(
            self.mode,
            mutants_options,
            test_options,
        )

    def update_command(self):
        return "cargo update"

    def env(self):
        return os.environ | self.env_vars

    def run(
        self,
        commit: Commit,
        individual_options_parent: tuple[Optional[str], Optional[str]],
        individual_options: tuple[Optional[str], Optional[str]],
    ) -> bool:
        """
        Run cargo mutants-rts.

        :return:
        """

        individual_mutants_options, individual_test_options = individual_options
        individual_mutants_options = individual_mutants_options if individual_mutants_options else ""
        individual_test_options = individual_test_options if individual_test_options else ""

        _LOGGER.info("About to start mutation testing using '" + self.mutants_command(individual_mutants_options, individual_test_options) + "' on " + self.repository.path)
        global ask_for_skip
        if ask_for_skip and input(" Skip? ") == "y":
            return True
        else:
            ask_for_skip = False

        # keep track of current working directory
        has_failed = False

        tmp_path = os.getcwd()

        os.makedirs(self.cache_dir, exist_ok=True)  # recursively create dirs

        # navigate into repo
        os.chdir(self.repository.path)

        ############################################################################################################
        # Run

        # checkout actual commit
        self.git_client.git_repo.git.checkout(commit.commit_str, force=True)
        self.git_client.git_repo.git.reset(commit.commit_str, hard=True)

        self.update_dependencies(commit)

        # run pre_hook if present
        if self.pre_hook:
            self.pre_hook()

        # prepare cache dir/file
        cache_file = "run_{}.log".format(int(time() * 1000))  # run identified by timestamp
        cache_file_path = os.path.join(self.cache_dir, cache_file)

        # Run test command on actual commit
        proc: SubprocessContainer = SubprocessContainer(
            command=self.mutants_command(individual_mutants_options, individual_test_options),
            output_filepath=cache_file_path,
            env=self.env(),
        )
        proc.execute(capture_output=True, shell=True)
        has_failed |= not (proc.exit_code == 0 or proc.exit_code == 2 or proc.exit_code == 3)

        # ******************************************************************************************************
        # Parse result

        result_matcher = re.search(
            r"^\d* mutants? tested in .*:(?: (\d*) missed,?)?(?: (\d*) caught,?)?(?: (\d*) unviable,?)?(?: (\d*) timeouts,?)?(?: (\d*) failed,?)?",
            proc.output,
            re.M,
        )

        missed = None
        caught = None
        unviable = None
        timeout = None
        failed = None

        if result_matcher is not None:
            missed = int(result_matcher.group(1)) if result_matcher.group(1) else 0
            caught = int(result_matcher.group(2)) if result_matcher.group(2) else 0
            unviable = int(result_matcher.group(3)) if result_matcher.group(3) else 0
            timeout = int(result_matcher.group(4)) if result_matcher.group(4) else 0
            failed = int(result_matcher.group(5)) if result_matcher.group(5) else 0

        test_report: MutantsReport = MutantsReport(
            name="mutants" + self.mode,
            duration=proc.end_to_end_time,
            mutants=[],
            commit=commit,
            commit_str=commit.commit_str,
            log=proc.output,
            has_failed=has_failed,
            missed=missed,
            caught=caught,
            timeout=timeout,
            unviable=unviable,
            failed=failed,
        )
        # create test report object

        with self.connection.create_session_ctx() as session:
            test_report = DBMutantsReport.create_or_update(report=test_report, session=session)
            session.commit()

        if not has_failed:
            # parse mutants
            loader = CargoMutantsTestReportLoader(self.repository.path + os.path.sep + "mutants.out")
            loader.load_mutants(test_report.id, self.connection)

        ############################################################################################################

        # return to previous directory
        os.chdir(tmp_path)

        self.git_client.git_repo.git.reset(commit.commit_str, hard=True)
        self.git_client.clean(rm_dirs=True)

        freed = gc.collect()
        _LOGGER.info("gc has freed " + str(freed) + " objects")

        return not has_failed

    def update_dependencies(self, commit):
        if not self.git_client.get_file_is_tracked(
            commit, "Cargo.lock"
        ):  # if Cargo.lock is versioned using git, we do not want to update all packages
            update_command = self.update_command()
            proc: SubprocessContainer = SubprocessContainer(
                command=update_command, output_filepath=self.prepare_cache_file()
            )
            proc.execute(capture_output=True, shell=True, timeout=100.0)
        else:
            _LOGGER.debug(
                "Found versioned Carg.lock, skipping update of all dependencies"
            )

        ## The following commands will just silently fail if not applicable

        # additionally update actix_derive which has shown to be problematic
        update_command = self.update_command() + " actix_derive --precise 0.6.0"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update chrono which has shown to be problematic
        update_command = self.update_command() + " chrono --precise 0.4.29"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update regex which has shown to be problematic
        update_command = self.update_command() + " regex --precise 1.4.3"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update proc-macro2@1 which has shown to be problematic in several projects
        update_command = self.update_command() + " proc-macro2@1"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update reedline which has shown to be problematic in meilisearch
        # update_command = self.update_command() + " reedline"
        # proc: SubprocessContainer = SubprocessContainer(
        #     command=update_command, output_filepath=self.prepare_cache_file()
        # )
        # proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update value-bag which has shown to be problematic in feroxbuster
        update_command = self.update_command() + " value-bag"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update log which has shown to be problematic in feroxbuster
        update_command = self.update_command() + " log"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update rustc-serialize which has shown to be problematic in zenoh
        update_command = self.update_command() + " rustc-serialize"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update log@0.4.14 which have shown to be problematic in zenoh
        versions = ["0.4.14"]
        for v in versions:
            update_command = self.update_command() + " log@" + v
            proc: SubprocessContainer = SubprocessContainer(
                command=update_command, output_filepath=self.prepare_cache_file()
            )
            proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update tokio which have shown to be problematic in penumbra
        update_command = self.update_command() + " tokio"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)

        # additionally update rustversion which have shown to be problematic in actix-web
        update_command = self.update_command() + " rustversion --precise 1.0.14"
        proc: SubprocessContainer = SubprocessContainer(
            command=update_command, output_filepath=self.prepare_cache_file()
        )
        proc.execute(capture_output=True, shell=True, timeout=100.0)
        
    def prepare_cache_file(self) -> str:
        # prepare cache dir/file
        cache_file = "run_{}.log".format(
            int(time() * 1000)
        )  # run identified by timestamp
        cache_file_path = os.path.join(self.cache_dir, cache_file)
        return cache_file_path
