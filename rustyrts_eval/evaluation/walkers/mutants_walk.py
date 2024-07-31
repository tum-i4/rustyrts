import logging
import shutil
import tempfile
from typing import Optional, Callable

from git import Repo

from .gitwalker import GivenWalkerStrategy, RandomWalkerStrategy, GitWalker
from ..hooks.cargo_mutants import CargoMutantsHook, RustyRTSMode
from ..hooks.scc import SccHook
from rustyrts_eval.models.scm.base import Repository
from ...models.scm.git import GitClient
from ...util.logging.logger import configure_logging_verbosity


def walk(
    connection,
    path,
    branch="main",
    logging_level="DEBUG",
    commits=None,
    env_vars: Optional[dict[str]] = None,
    options: Optional[list[str]] = None,
    pre_hook: Optional[Callable] = None,
):
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

    # create repo
    repository = Repository(path=path, repository_type="git")
    git_client = GitClient(repository)

    # If a commit is added to the repositories, the seed responsible for making the evaluation reproducible
    # does not work correctly anymore
    # that is why we fixed the commits that are analyzed
    (strategy, num_commits) = (GivenWalkerStrategy(commits), len(commits)) if commits else (RandomWalkerStrategy(repository, branch=branch), 20)

    options = options if options else []
    options.append("--json")
    # options.append("--gitignore=false")
    options.append("--in-place")

    env_vars = env_vars if env_vars else {}

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
            CargoMutantsHook(
                repository=repository,
                git_client=git_client,
                mode=RustyRTSMode.TEST,
                env_vars=env_vars,
                options=options,
                connection=connection,
                pre_hook=pre_hook,
            ),
            CargoMutantsHook(
                repository=repository,
                git_client=git_client,
                mode=RustyRTSMode.BASIC,
                env_vars=env_vars,
                options=options,
                connection=connection,
                pre_hook=pre_hook,
            ),
            CargoMutantsHook(
                repository=repository,
                git_client=git_client,
                mode=RustyRTSMode.DYNAMIC,
                env_vars=env_vars,
                options=options,
                connection=connection,
                pre_hook=pre_hook,
            ),
            CargoMutantsHook(
                repository=repository,
                git_client=git_client,
                mode=RustyRTSMode.STATIC,
                env_vars=env_vars,
                options=options,
                connection=connection,
                pre_hook=pre_hook,
            ),
        ],
    )
    # create walker

    # start walking
    walker.walk()

    # cleanup
    if tmp_path is not None:
        shutil.rmtree(tmp_path)
