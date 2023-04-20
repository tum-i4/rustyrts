import logging
import shutil
import tempfile

from git import Repo

from rts_eval.db.base import DBConnection
from rts_eval.evaluation.git_walker import GivenWalkerStrategy, RandomWalkerStrategy, GitWalker
from rts_eval.evaluation.hooks.cargo_mutants import CargoMutantsHook, RustyMutantsRTSMode
from rts_eval.evaluation.hooks.scc import SccHook
from rts_eval.models.scm.base import Repository
from rts_eval.models.scm.git import GitClient
from rts_eval.util.logging.logger import configure_logging_verbosity

db_url = "postgresql://postgres@localhost:5432/rustyrts"


def walk(path, branch="main", logging_level="DEBUG", commits=None):
    # set logging level
    numeric_level = getattr(logging, logging_level.upper(), None)
    if not isinstance(numeric_level, int):
        raise ValueError(f"Invalid log level: {logging_level}")
    configure_logging_verbosity(numeric_level)

    # if you want to clone a remote repository
    path = path
    tmp_path = None
    if ".git" in path:
        tmp_path = tempfile.mkdtemp()
        Repo.clone_from(url=path, to_path=tmp_path)
        path = tmp_path

    # create DB connection
    connection = DBConnection(url=db_url)

    # create repo
    repository = Repository(path=path, repository_type="git")
    git_client = GitClient(repository)

    # If a commit is added to the repositories, the seed responsible for making the evaluation reproducible
    # does not work correctly anymore
    # that is why we fixed the commits that are analyzed
    (strategy, num_commits) = (GivenWalkerStrategy(commits), len(commits)) if commits else (
        RandomWalkerStrategy(repository, branch=branch), 20)

    walker = GitWalker(
        repository=repository,
        connection=connection,
        strategy=strategy,
        num_commits=num_commits,

        hooks=[
            # scc
            SccHook(
                repository=repository,
                connection=connection,
                language="Rust",
            ),

            CargoMutantsHook(repository=repository,
                             git_client=git_client,
                             mode=RustyMutantsRTSMode.TEST,
                             env_vars={"RUSTFLAGS": " ".join(["--cap-lints=allow", "-C", "link-arg=-fuse-ld=mold"])},
                             options=["--json"],
                             connection=connection
                             ),

            CargoMutantsHook(repository=repository,
                             git_client=git_client,
                             mode=RustyMutantsRTSMode.DYNAMIC,
                             env_vars={"RUSTFLAGS": " ".join(["--cap-lints=allow", "-C", "link-arg=-fuse-ld=mold"])},
                             options=["--json"],
                             connection=connection
                             ),

            CargoMutantsHook(repository=repository,
                             git_client=git_client,
                             mode=RustyMutantsRTSMode.STATIC,
                             env_vars={"RUSTFLAGS": " ".join(["--cap-lints=allow", "-C", "link-arg=-fuse-ld=mold"])},
                             options=["--json"],
                             connection=connection
                             )
        ],
    )
    # create walker

    # start walking
    walker.walk()

    # cleanup
    if tmp_path is not None:
        shutil.rmtree(tmp_path)
