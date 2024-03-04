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


class RustyMutantsRTSMode(str, Enum):
    TEST = ""
    DYNAMIC = " dynamic"
    STATIC = " static"


class CargoMutantsHook(Hook):
    def __init__(
        self,
        repository: Repository,
        git_client: GitClient,
        mode: RustyMutantsRTSMode,
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
        self.options = options if options else []
        self.test_options = test_options if test_options else []
        self.connection = connection
        self.pre_hook = pre_hook

    def mutants_command(self):
        return "cargo mutants-rts{0} {1} -- {2}".format(
            self.mode,
            " ".join(self.options),
            " ".join(self.test_options),
        )

    def env(self):
        return os.environ | self.env_vars

    def run(
        self, commit: Commit, features_parent: Optional[str], features: Optional[str]
    ) -> bool:
        """
        Run cargo mutants-rts.

        :return:
        """

        _LOGGER.info(
            "About to start mutation testing using '"
            + self.mutants_command()
            + "' on "
            + self.repository.path
        )
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

        # run pre_hook if present
        if self.pre_hook:
            self.pre_hook()

        # prepare cache dir/file
        cache_file = "run_{}.log".format(
            int(time() * 1000)
        )  # run identified by timestamp
        cache_file_path = os.path.join(self.cache_dir, cache_file)

        # Run test command on actual commit
        proc: SubprocessContainer = SubprocessContainer(
            command=self.mutants_command(),
            output_filepath=cache_file_path,
            env=self.env(),
        )
        proc.execute(capture_output=True, shell=True)
        has_failed |= not (
            proc.exit_code == 0 or proc.exit_code == 2 or proc.exit_code == 3
        )

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
            test_report = DBMutantsReport.create_or_update(
                report=test_report, session=session
            )
            _LOGGER.warning("Mutants " + str(test_report.mutants))
            session.commit()

        if not has_failed:
            # parse mutants
            loader = CargoMutantsTestReportLoader(
                self.repository.path + os.path.sep + "mutants.out"
            )
            loader.load_mutants(test_report.id, self.connection)

        ############################################################################################################

        # return to previous directory
        os.chdir(tmp_path)

        self.git_client.git_repo.git.reset(commit.commit_str, hard=True)
        self.git_client.clean(rm_dirs=True)

        freed = gc.collect()
        _LOGGER.info("gc has freed " + str(freed) + " objects")

        return not has_failed
