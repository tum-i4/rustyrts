import logging
import shutil
import tempfile
from typing import Optional

from git import Repo

from rts_eval.cli.db.commands import dump, _dump
from rts_eval.db.base import DBConnection
from rts_eval.evaluation.git_walker import GivenWalkerStrategy, RandomWalkerStrategy, GitWalker
from rts_eval.evaluation.hooks.scc import SccHook
from rts_eval.evaluation.hooks.cargo_test import CargoTestHook
from rts_eval.evaluation.hooks.cargo_rustyrts import CargoRustyRTSHook, RustyRTSMode
from rts_eval.models.scm.base import Repository
from rts_eval.models.scm.git import GitClient
from rts_eval.util.logging.logger import configure_logging_verbosity

db_url = "postgresql://postgres:rustyrts@localhost:5432/git"


def walk(path, branch="main", logging_level="DEBUG", commits: Optional[list[(str, Optional[str], Optional[str])]] = None,
         env_vars: Optional[dict[str]] = None,
         build_options: Optional[list[str]] = None,
         test_options: Optional[list[str]] = None,
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

    # create DB connection
    connection = DBConnection(url=db_url, pool_pre_ping=True)

    # create repo
    repository = Repository(path=path, repository_type="git")
    git_client = GitClient(repository)

    # If a commit is added to the repositories, the seed responsible for making the evaluation reproducible
    # does not work correctly anymore
    # that is why we fixed the commits that are analyzed
    (strategy, num_commits) = (GivenWalkerStrategy(commits), len(commits)) if commits else (
        RandomWalkerStrategy(repository, branch=branch), 30)

    build_options = build_options if build_options else []
    build_options += ["-Z no-index-update"]

    test_options = test_options if test_options else []
    test_options += ["-Z unstable-options", "--report-time", "--format", "json"]

    env_vars = env_vars if env_vars else {}
    env_vars |= {"RUSTFLAGS": " ".join(["--cap-lints=allow", "-C", "link-arg=-fuse-ld=lld"])}

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

            # ***********************************************************************************************************
            # Multi threaded

            CargoTestHook(repository=repository,
                          connection=connection,
                          git_client=git_client,
                          report_name="cargo test",
                          env_vars=env_vars.copy(),
                          build_options=build_options.copy(),
                          test_options=test_options.copy()
                          ),

            CargoRustyRTSHook(repository=repository,
                              connection=connection,
                              git_client=git_client,
                              report_name="cargo rustyrts dynamic",
                              mode=RustyRTSMode.DYNAMIC,
                              env_vars=env_vars.copy(),
                              build_options=build_options.copy(),
                              test_options=test_options.copy()
                              ),

            CargoRustyRTSHook(repository=repository,
                              connection=connection,
                              git_client=git_client,
                              report_name="cargo rustyrts static",
                              mode=RustyRTSMode.STATIC,
                              env_vars=env_vars.copy(),
                              build_options=build_options.copy(),
                              test_options=test_options.copy()
                              ),

            # ***********************************************************************************************************
            # Single threaded

#            CargoTestHook(repository=repository,
#                          connection=connection,
#                          git_client=git_client,
#                          report_name="cargo test single threaded",
#                          env_vars=env_vars.copy(),
#                          build_options=build_options.copy() + ["--jobs 1"],
#                          test_options=test_options.copy() + ["--test-threads 1"]
#                          ),
#
#            CargoRustyRTSHook(repository=repository,
#                              connection=connection,
#                              git_client=git_client,
#                              report_name="cargo rustyrts dynamic single threaded",
#                              mode=RustyRTSMode.DYNAMIC,
#                              env_vars=env_vars.copy(),
#                              build_options=build_options.copy() + ["--jobs 1"],
#                              test_options=test_options.copy() + ["--test-threads 1"]
#                              ),
#
#            CargoRustyRTSHook(repository=repository,
#                              connection=connection,
#                              git_client=git_client,
#                              report_name="cargo rustyrts static single threaded",
#                              mode=RustyRTSMode.STATIC,
#                              env_vars=env_vars.copy(),
#                              build_options=build_options.copy() + ["--jobs 1"],
#                              test_options=test_options.copy() + ["--test-threads 1"]
#                              ),
        ],
    )
    # create walker

    # start walking
    walker.walk()

    # cleanup
    if tmp_path is not None:
        shutil.rmtree(tmp_path)

    # backup
    _dump(connection, False, "post_" + repository.path[repository.path.rfind("/")+1:])