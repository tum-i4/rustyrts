import json
import os
import tempfile
from typing import Optional
from time import time

from ...models.scm.git import GitClient

from ..hooks.base import Hook
from ...db.base import DBConnection
from ...db.git import DBCommit
from ...models.scm.base import Commit, Repository
from ...util.logging.logger import get_logger
from ...util.os.exec import SubprocessContainer

_LOGGER = get_logger(__name__)


class SccHook(Hook):
    def __init__(
        self,
        repository: Repository,
        connection: DBConnection,
        language: str,
        output_path: Optional[str] = None,
    ):
        super().__init__(repository, None, git_client=GitClient(repository))
        self.connection = connection
        self.language = language
        if output_path:
            self.cache_dir = os.path.join(self.output_path, ".scc-hook")
        else:
            self.cache_dir = os.path.join(tempfile.gettempdir(), ".scc-hook")

    def run(
        self,
        commit: Commit,
        features_parent: Optional[str],
        features: Optional[str],
        rustflags: Optional[str],
    ) -> bool:
        _LOGGER.debug("Checking out commit {}.".format(commit.commit_str))
        self.git_client.checkout(commit)

        with self.connection.create_session_ctx() as session:
            os.makedirs(self.cache_dir, exist_ok=True)

            # prepare cache dir/file
            cache_file = "run_{}.log".format(int(time() * 1000))  # run identified by timestamp
            cache_file_path = os.path.join(self.cache_dir, cache_file)

            command = "scc " + self.repository.path + " -f json"

            proc: SubprocessContainer = SubprocessContainer(command=command, output_filepath=cache_file_path)
            proc.execute(capture_output=True, shell=True, timeout=1000.0)

            result = json.loads(proc.output)

            data = list(filter(lambda x: x["Name"] == self.language, result))

            if data:
                commit.nr_lines = data[0]["Code"]
                commit.nr_files = data[0]["Count"]
            else:
                commit.nr_files = 0
                commit.nr_lines = 0

            DBCommit.create_or_update(commit=commit, session=session)
            # commit object
            session.commit()

        # reset changes and clean untracked files
        # (keep dirs, as they might be required for caching)
        self.git_client.reset_hard()
        self.git_client.clean(rm_dirs=False)

        return True
