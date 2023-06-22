import random
from typing import Optional, List

from git import Repo

from .base import Walker, Hook, WalkerStrategy
from ..db.git import DBCommit
from ..models.scm.base import Repository
from ..models.scm.git import GitClient
from ..util.logging.logger import get_logger

_LOGGER = get_logger(__name__)


class SequentialWalkerStrategy(WalkerStrategy):
    def __init__(self, repository, include_merge_commits=False, branch: str = "main"):
        self.git_repo: Repo = Repo(repository.path)

        self.include_merge_commits = include_merge_commits
        self.branch = branch

    def commits(self) -> List[str]:
        start_commit = self.git_repo.git.rev_list(self.branch, max_parents=0).splitlines()[0]
        return [commit.hexsha for commit in self.git_repo.iter_commits(
            "{}..{}".format(start_commit, self.branch),
            ancestry_path=True,
            no_merges=(not self.include_merge_commits))]

    def __iter__(self):
        return self.commits().__iter__()


class RandomWalkerStrategy(SequentialWalkerStrategy):
    def __init__(self, repository, include_merge_commits=False, branch: str = "main"):
        super().__init__(repository, include_merge_commits, branch)

    def __iter__(self):
        commits = super().commits()
        random.seed(42)
        random.shuffle(commits)
        return commits.__iter__()


class GivenWalkerStrategy(WalkerStrategy):
    def __init__(self, commits: List[str]):
        self.commits = commits

    def __iter__(self):
        return self.commits.__iter__()


class GitWalker(Walker):
    """
       GitWalker class to replay git repository history.
       """

    def __init__(
            self,
            repository: Repository,
            connection,
            strategy: WalkerStrategy,
            num_commits: Optional[int] = 10,
            hooks: Optional[List[Hook]] = None,
    ):
        super().__init__(
            repository,
            strategy,
            num_commits,
            hooks
        )
        self.git_client: GitClient = GitClient(repository=repository)
        self.connection = connection

    def walk(self) -> None:
        # clean for convenience
        self.git_client.reset_hard()
        self.git_client.clean(rm_dirs=True)

        # init counter
        counter = 0

        for commit in self.strategy:
            # get next commit with changeset
            next_commit = self.git_client.get_commit_from_repo(commit_id=commit)

            # write commit to DB
            session = self.connection.create_session()
            DBCommit.create_or_update(commit=next_commit, session=session)
            session.commit()

            # run hooks
            success = True
            for h in self.hooks:
                success &= h.run(next_commit)
                if not success:
                    break

            # inc counter and break if `num_commits` reached
            if success:
                counter += 1
            if counter >= self.num_commits:
                break
